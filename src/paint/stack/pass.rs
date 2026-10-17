//! Per-call helpers of the compositor: what one pass works on, reusable buffers, per-pixel layer weights and
//! the blend step used inside a layer's content stack.

use crate::paint::blend::BlendMode;
use crate::paint::channel::Channel;
use crate::paint::image::Rect;
use crate::paint::layer::Layer;
use crate::paint::working_space::SpaceConverter;

/// What one `composite_rect` call works on: the channel, the rect and the working space.
pub(super) struct Pass {
    pub channel: Channel,
    pub r: Rect,
    pub space: SpaceConverter,
}

impl Pass {
    /// The channel default with coverage 0, in working space.
    pub fn empty(&self) -> [f32; 4] {
        let d = self.channel.default_value();
        self.space.to_working([d[0], d[1], d[2], 0.0])
    }

    /// The same pass over another rectangle.
    pub fn with_rect(&self, r: Rect) -> Pass {
        Pass { channel: self.channel, r, space: self.space.clone() }
    }
}

/// Reusable per-tile buffers.
#[derive(Default)]
pub(super) struct Scratch {
    pub top: Vec<[f32; 4]>,
}

/// Per-pixel layer strength over a rect: uniform, or uniform × mask.
pub(super) enum Weights {
    Uniform(f32),
    Masked(Vec<f32>),
}

impl Weights {
    pub fn at(&self, i: usize) -> f32 {
        match self {
            Weights::Uniform(w) => *w,
            Weights::Masked(m) => m.get(i).copied().unwrap_or(0.0),
        }
    }
}

/// True if the layer is visible and has an enabled blend entry for the channel.
pub(super) fn affects(l: &Layer, channel: Channel) -> bool {
    l.visible && l.blend_for(channel).is_some_and(|b| b.mode != BlendMode::Disable)
}

/// One step of a content stack: `top` (alpha = coverage) blended over `below` with `mode` at `opacity`.
/// Where nothing is below yet (coverage 0) the top's own value is taken with its coverage, so the content
/// stays straight (not premultiplied) and the mode of the first effect does not darken it.
pub(super) fn stack_mix(mode: BlendMode, below: [f32; 4], top: [f32; 4], opacity: f32) -> [f32; 4] {
    if below[3] > 0.0 || mode == BlendMode::Disable {
        return mode.mix(below, top, opacity);
    }
    let unit = |v: f32| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
    let w = unit(opacity) * unit(top[3]);
    if w <= 0.0 { below } else { [top[0], top[1], top[2], w] }
}

/// `r` grown by `radius` on every side, clipped to a `size` texture.
pub(super) fn grow(r: Rect, radius: u32, size: (u32, u32)) -> Rect {
    let x = r.x.saturating_sub(radius);
    let y = r.y.saturating_sub(radius);
    let x1 = (r.x + r.w).saturating_add(radius).min(size.0.max(r.x + r.w));
    let y1 = (r.y + r.h).saturating_add(radius).min(size.1.max(r.y + r.h));
    Rect { x, y, w: x1 - x, h: y1 - y }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_effect_keeps_its_value_and_grow_clips() {
        let empty = [0.5, 0.5, 0.5, 0.0];
        assert_eq!(stack_mix(BlendMode::Multiply, empty, [0.2, 0.2, 0.2, 1.0], 0.5), [0.2, 0.2, 0.2, 0.5]);
        assert_eq!(stack_mix(BlendMode::Normal, empty, [0.2, 0.2, 0.2, 0.0], 1.0), empty);
        assert_eq!(stack_mix(BlendMode::Disable, empty, [0.2; 4], 1.0), empty);
        let full = [0.5, 0.5, 0.5, 1.0];
        assert_eq!(stack_mix(BlendMode::Multiply, full, [0.5, 0.5, 0.5, 1.0], 1.0), [0.25, 0.25, 0.25, 1.0]);
        let r = Rect { x: 2, y: 0, w: 4, h: 4 };
        assert_eq!(grow(r, 3, (8, 8)), Rect { x: 0, y: 0, w: 8, h: 7 });
    }
}
