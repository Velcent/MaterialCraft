//! Per-call helpers of the compositor: what one pass works on, reusable buffers, and per-pixel layer weights.

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

/// The layer's strength over `r`, or `None` where it has no effect at all.
pub(super) fn layer_weights(l: &Layer, strength: f32, r: Rect) -> Option<Weights> {
    let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
    if strength <= 0.0 {
        return None;
    }
    let Some(mask) = &l.mask else {
        return Some(Weights::Uniform(strength));
    };
    let img = mask.image();
    if !img.region_allocated(r) {
        return None; // unallocated mask tiles read as zero: fully masked out
    }
    let mut px = vec![[0.0f32; 4]; r.area()];
    img.read_region(r, &mut px);
    Some(Weights::Masked(px.iter().map(|p| (p[0] * p[3]).clamp(0.0, 1.0) * strength).collect()))
}
