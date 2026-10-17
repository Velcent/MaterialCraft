//! [`Mask`]: where a layer applies. A mask is a grey value per texel: its initial value with an ordered
//! stack of [`MaskEffect`]s on top (bottom to top), clamped to 0..1 after each effect. The value scales the
//! layer's strength. A disabled mask is kept but does nothing.

use super::effect::{EffectKind, GreyBlend, MaskEffect, MaskPixels};
use super::image::TiledImage;

#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    pub enabled: bool,
    /// The value the mask starts from, 0 (black) … 1 (white).
    pub initial: f32,
    /// Bottom → top.
    pub effects: Vec<MaskEffect>,
}

impl Mask {
    /// An enabled mask starting from `initial` without effects.
    pub fn new(initial: f32) -> Mask {
        Mask { enabled: true, initial, effects: Vec::new() }
    }

    /// A painted mask: black with `image` painted on top at full strength, so the value is red × alpha
    /// (the image masks of format 0.2).
    pub fn from_image(image: TiledImage) -> Mask {
        let paint = MaskEffect::new(GreyBlend::default(), EffectKind::Paint(MaskPixels { image }));
        Mask { effects: vec![paint], ..Mask::new(0.0) }
    }

    /// Builder: adds `effect` on top.
    pub fn with_effect(mut self, effect: MaskEffect) -> Mask {
        self.effects.push(effect);
        self
    }

    /// Every image of the mask's paint effects with the effect's index.
    pub fn images(&self) -> impl Iterator<Item = (usize, &TiledImage)> {
        self.effects.iter().enumerate().filter_map(|(i, e)| match &e.kind {
            EffectKind::Paint(p) => Some((i, &p.image)),
            _ => None,
        })
    }

    /// The image of paint effect `index`.
    pub fn image_mut(&mut self, index: usize) -> Option<&mut TiledImage> {
        match &mut self.effects.get_mut(index)?.kind {
            EffectKind::Paint(p) => Some(&mut p.image),
            _ => None,
        }
    }

    /// The value everywhere when the mask is the same on every texel: no effects other than uniform fills,
    /// anchors, switched-off effects and paint effects with nothing painted. `None` otherwise. A disabled
    /// mask is 1 (it does not restrict the layer).
    pub fn constant_value(&self) -> Option<f32> {
        if !self.enabled {
            return Some(1.0);
        }
        let mut value = unit(self.initial);
        for e in self.effects.iter().filter(|e| e.enabled) {
            let top = match &e.kind {
                EffectKind::Fill(v) => [*v, *v, *v, 1.0],
                EffectKind::Paint(p) if p.image.allocated_tiles() == 0 => {
                    let coverage = if p.image.format().layout.has_alpha() { 0.0 } else { 1.0 };
                    [0.0, 0.0, 0.0, coverage]
                }
                EffectKind::Anchor(_) => continue,
                _ => return None,
            };
            value = mix_grey(e.blend, value, top);
        }
        Some(value)
    }

    /// True when the mask leaves the layer untouched everywhere (disabled or constant white).
    pub fn is_identity(&self) -> bool {
        self.constant_value().is_some_and(|v| v >= 1.0)
    }
}

/// One step of a mask stack: `top` (grey in red, alpha = coverage) blended over `below`, clamped to 0..1.
pub fn mix_grey(blend: GreyBlend, below: f32, top: [f32; 4]) -> f32 {
    unit(blend.mode.mix([below, below, below, 1.0], top, blend.opacity)[0])
}

/// Clamps to 0..1; NaN becomes 0.
fn unit(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{BlendMode, PixelFormat};

    #[test]
    fn constant_masks_and_painted_ones() {
        assert_eq!(Mask::new(1.0).constant_value(), Some(1.0));
        assert!(Mask::new(1.0).is_identity());
        let mut off = Mask::new(0.0);
        off.enabled = false;
        assert!(off.is_identity());
        let half = Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend { mode: BlendMode::Normal, opacity: 0.5 }, EffectKind::Fill(1.0)));
        assert_eq!(half.constant_value(), Some(0.5));
        let mul = half.clone().with_effect(MaskEffect::new(GreyBlend { mode: BlendMode::Multiply, opacity: 1.0 }, EffectKind::Fill(0.5)));
        assert_eq!(mul.constant_value(), Some(0.25));
        let empty = TiledImage::new(4, 4, PixelFormat::from_id("graya8").unwrap()).unwrap();
        assert_eq!(Mask::from_image(empty.clone()).constant_value(), Some(0.0));
        let mut painted = empty;
        painted.set(1, 1, [1.0; 4]);
        let m = Mask::from_image(painted);
        assert_eq!(m.constant_value(), None);
        assert_eq!(m.images().count(), 1);
        assert_eq!(mix_grey(GreyBlend::default(), 0.0, [0.8, 0.8, 0.8, 0.5]), 0.4);
        assert_eq!(mix_grey(GreyBlend { mode: BlendMode::Add, opacity: 1.0 }, 0.7, [0.8, 0.8, 0.8, 1.0]), 1.0);
    }
}
