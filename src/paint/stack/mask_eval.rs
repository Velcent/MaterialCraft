//! A layer's strength over a rectangle: its opacity times its mask. The mask is its initial value with its
//! effects blended on top (see [`crate::paint::mask`]); effects the compositor cannot evaluate are left out.

use super::Compositor;
use super::below::LayerStack;
use super::pass::Weights;
use super::procedural::{FilterRun, generate};
use crate::paint::effect::{EffectKind, EvalTarget, MaskEffect};
use crate::paint::image::Rect;
use crate::paint::layer::Layer;
use crate::paint::mask::{Mask, mix_grey};

impl Compositor<'_> {
    /// The layer's strength over `r`, or `None` where it has no effect at all.
    pub(super) fn layer_weights(&self, l: &Layer, strength: f32, r: Rect) -> Option<Weights> {
        let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
        if strength <= 0.0 {
            return None;
        }
        let Some(mask) = l.mask.as_ref().filter(|m| m.enabled) else {
            return Some(Weights::Uniform(strength));
        };
        if let Some(v) = mask.constant_value() {
            return (v > 0.0).then_some(Weights::Uniform(v * strength));
        }
        let grey = self.mask_values(l, mask, mask.effects.len(), r);
        if grey.iter().all(|g| *g <= 0.0) {
            return None;
        }
        Some(Weights::Masked(grey.into_iter().map(|g| g * strength).collect()))
    }

    /// Whether the compositor can evaluate the mask effect.
    pub(super) fn mask_effect_computable(&self, e: &MaskEffect) -> bool {
        match &e.kind {
            EffectKind::Paint(_) | EffectKind::Fill(_) | EffectKind::Anchor(_) => true,
            EffectKind::Generator(p) => self.effects.generator(&p.id).is_some(),
            EffectKind::Filter(p) => self.effects.filter(&p.id).is_some(),
            _ => false,
        }
    }

    /// The grey values of `layer`'s mask over `r` with its first `upto` effects.
    pub(super) fn mask_values(&self, layer: &Layer, mask: &Mask, upto: usize, r: Rect) -> Vec<f32> {
        let initial = if mask.initial.is_finite() { mask.initial.clamp(0.0, 1.0) } else { 0.0 };
        let mut grey = vec![initial; r.area()];
        for (index, e) in mask.effects.iter().enumerate().take(upto).filter(|(_, e)| e.enabled) {
            let Some(top) = self.mask_top(layer, mask, index, e, r, &grey) else { continue };
            for (g, t) in grey.iter_mut().zip(top) {
                *g = mix_grey(e.blend, *g, t);
            }
        }
        grey
    }

    /// What mask effect `index` puts on top over `r` (grey in red, alpha = coverage); `below` is the mask so far.
    fn mask_top(&self, layer: &Layer, mask: &Mask, index: usize, e: &MaskEffect, r: Rect, below: &[f32]) -> Option<Vec<[f32; 4]>> {
        match &e.kind {
            EffectKind::Paint(p) => {
                if p.image.format().layout.has_alpha() && !p.image.region_allocated(r) {
                    return None; // nothing painted here
                }
                let mut px = vec![[0.0; 4]; r.area()];
                p.image.read_region(r, &mut px);
                Some(px)
            }
            EffectKind::Fill(v) => Some(vec![[*v, *v, *v, 1.0]; r.area()]),
            EffectKind::Generator(p) => generate(self, p, EvalTarget::Mask, r, Some(&LayerStack { compositor: self, layer })),
            EffectKind::Filter(p) => {
                let run = FilterRun { compositor: self, procedural: p, target: EvalTarget::Mask, r };
                run.run(|input| {
                    let grey = if input == r { below.to_vec() } else { self.mask_values(layer, mask, index, input) };
                    grey.into_iter().map(|g| [g, g, g, 1.0]).collect()
                })
            }
            _ => None,
        }
    }
}
