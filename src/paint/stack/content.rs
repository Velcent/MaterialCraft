//! A layer's content for one channel: its base (paint image, uniform fill or a group's isolated composite)
//! with its content effects on top, in working space, straight RGBA with alpha = coverage.

use super::Compositor;
use super::fill_eval::FillTop;
use super::pass::{Pass, Scratch, stack_mix};
use super::procedural::{FilterRun, generate};
use crate::paint::blend::BlendMode;
use crate::paint::channel::Channel;
use crate::paint::effect::{ContentEffect, EffectKind, EvalTarget};
use crate::paint::layer::{FillSource, Layer, LayerKind};
use crate::paint::validate::MAX_GROUP_DEPTH;

impl Compositor<'_> {
    /// True if an enabled content effect the compositor can evaluate changes `pass.channel`.
    pub(super) fn has_active_effects(&self, l: &Layer, pass: &Pass) -> bool {
        l.effects.iter().any(|e| self.content_effect_applies(e, pass) && self.content_effect_computable(e) && !is_noop(e, pass))
    }

    /// Enabled and with a blend entry for the pass's channel that is not `Disable`.
    pub(super) fn content_effect_applies(&self, e: &ContentEffect, pass: &Pass) -> bool {
        e.enabled && e.blend_for(pass.channel).is_some_and(|b| b.mode != BlendMode::Disable)
    }

    /// Whether the compositor can evaluate the effect (built in, or a registered generator/filter).
    pub(super) fn content_effect_computable(&self, e: &ContentEffect) -> bool {
        match &e.kind {
            EffectKind::Fill(f) => self.fill_computable(f).is_ok(),
            EffectKind::PolygonFill(p) => self.picks_computable() && self.fill_computable(&p.values).is_ok(),
            EffectKind::Reference(r) => self.reference_computable(r),
            EffectKind::ColorSelection(_) => false,
            k if k.is_builtin() => true,
            EffectKind::Generator(p) => self.effects.generator(&p.id).is_some(),
            EffectKind::Filter(p) => self.effects.filter(&p.id).is_some(),
            _ => false,
        }
    }

    /// The content of `l` with its first `upto` effects over `pass.r`. An effect layer's content starts from the
    /// composite below it ([`Self::stack_below`]), any other layer's from nothing.
    pub(super) fn content(&self, l: &Layer, upto: usize, pass: &Pass, depth: usize) -> Vec<[f32; 4]> {
        let start = match is_effect_layer(l, pass.channel) {
            true => self.stack_below(l, pass),
            false => vec![pass.empty(); pass.r.area()],
        };
        self.content_over(l, upto, pass, depth, start)
    }

    /// The content of `l` with its first `upto` effects, starting from `c`.
    pub(super) fn content_over(&self, l: &Layer, upto: usize, pass: &Pass, depth: usize, mut c: Vec<[f32; 4]>) -> Vec<[f32; 4]> {
        if is_effect_layer(l, pass.channel) {
            let mut base = vec![pass.empty(); pass.r.area()];
            self.base(l, pass, &mut base, depth);
            for (a, b) in c.iter_mut().zip(base) {
                *a = BlendMode::Normal.mix(*a, b, 1.0);
            }
        } else {
            self.base(l, pass, &mut c, depth);
        }
        for (index, e) in l.effects.iter().enumerate().take(upto) {
            if self.content_effect_applies(e, pass) {
                self.apply_content_effect(l, index, e, pass, &mut c, depth);
            }
        }
        c
    }

    fn base(&self, l: &Layer, pass: &Pass, c: &mut [[f32; 4]], depth: usize) {
        match &l.kind {
            LayerKind::Paint(p) => {
                let Some(img) = p.image(pass.channel) else { return };
                if img.format().layout.has_alpha() && !img.region_allocated(pass.r) {
                    return;
                }
                img.read_region(pass.r, c);
                pass.space.to_working_all(c);
            }
            LayerKind::Fill(f) => match self.fill_top(f, pass) {
                Some(FillTop::Constant(v)) => c.fill(v),
                Some(FillTop::Pixels(px)) => {
                    for (c, p) in c.iter_mut().zip(px) {
                        *c = p;
                    }
                }
                None => {}
            },
            LayerKind::Group(children) => {
                if depth < MAX_GROUP_DEPTH {
                    self.apply_layers(children, pass, c, depth + 1, &mut Scratch::default());
                }
            }
            // Resolved before compositing (`Document::resolved_set`); one left here is broken.
            LayerKind::Instance(_) => {}
        }
    }

    fn apply_content_effect(&self, l: &Layer, index: usize, e: &ContentEffect, pass: &Pass, c: &mut [[f32; 4]], depth: usize) {
        let Some(blend) = e.blend_for(pass.channel).copied() else { return };
        let top: Vec<[f32; 4]> = match &e.kind {
            EffectKind::Paint(p) => {
                let Some(img) = p.image(pass.channel) else { return };
                if img.format().layout.has_alpha() && !img.region_allocated(pass.r) {
                    return;
                }
                let mut px = vec![[0.0; 4]; pass.r.area()];
                img.read_region(pass.r, &mut px);
                pass.space.to_working_all(&mut px);
                px
            }
            EffectKind::Fill(f) => match self.fill_top(f, pass) {
                Some(top) => top.into_pixels(pass.r.area()),
                None => return,
            },
            EffectKind::Generator(p) => match generate(self, p, EvalTarget::Channel(pass.channel), pass.r, None) {
                Some(px) => px,
                None => return,
            },
            EffectKind::Filter(p) => {
                let run = FilterRun { compositor: self, procedural: p, target: EvalTarget::Channel(pass.channel), r: pass.r };
                match run.run(|input| if input == pass.r { c.to_vec() } else { self.content(l, index, &pass.with_rect(input), depth) }) {
                    Some(px) => px,
                    None => return,
                }
            }
            EffectKind::Levels(levels) => self.levels_top(levels, pass, c),
            EffectKind::PolygonFill(p) => match self.polygon_fill_top(p, pass) {
                Some(px) => px,
                None => return,
            },
            EffectKind::Reference(r) => match self.reference_content(r, pass) {
                Some(px) => px,
                None => return,
            },
            _ => return,
        };
        for (a, t) in c.iter_mut().zip(top) {
            *a = stack_mix(blend.mode, *a, t, blend.opacity);
        }
    }
}

/// True for a layer that is not a folder and blends `channel` with `Passthrough`: an effect layer, whose effects
/// work on the composite below it.
pub(super) fn is_effect_layer(l: &Layer, channel: Channel) -> bool {
    !matches!(l.kind, LayerKind::Group(_)) && l.blend_for(channel).is_some_and(|b| b.mode == BlendMode::Passthrough)
}

/// Effects that leave the channel as it is: paint without an image for it, fills without a value for it,
/// anchors.
fn is_noop(e: &ContentEffect, pass: &Pass) -> bool {
    match &e.kind {
        EffectKind::Paint(p) => p.image(pass.channel).is_none(),
        EffectKind::Fill(f) => match &f.source {
            FillSource::Uniform(_) | FillSource::Image(_) | FillSource::Texture(_) => !f.channels().contains(&pass.channel),
            FillSource::Material { .. } => false,
        },
        EffectKind::Anchor(_) => true,
        EffectKind::Levels(l) => l.is_identity(),
        _ => false,
    }
}
