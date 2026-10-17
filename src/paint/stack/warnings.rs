//! [`CompositeWarning`]: what compositing a channel cannot do exactly, found by walking the stack without
//! evaluating it.

use super::Compositor;
use super::fill_eval::FillProblem;
use super::pass::affects;
use crate::paint::blend::BlendMode;
use crate::paint::channel::Channel;
use crate::paint::effect::EffectKind;
use crate::paint::layer::{Layer, LayerImage, LayerKind};
use crate::paint::validate::MAX_GROUP_DEPTH;
use crate::projection::NotComputable;

/// Something the compositor could not do exactly. Layers are named by id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeWarning {
    /// A material fill cannot be evaluated by the core; the layer was left out.
    MaterialFillSkipped { layer: String },
    /// A group nested deeper than [`MAX_GROUP_DEPTH`]; it was left out.
    GroupTooDeep { layer: String },
    /// A paint image or mask does not match the texture set resolution; it was read as is (cropped / zero-padded).
    ImageSizeMismatch { layer: String },
    /// An effect the compositor cannot evaluate (no implementation registered, or a kind not computable yet)
    /// was left out. `index` counts in the layer's content stack, or its mask stack if `in_mask`.
    EffectNotComputable { layer: String, in_mask: bool, index: usize, kind: String },
    /// Content effects on a pass-through group are not evaluated.
    GroupEffectsSkipped { layer: String },
    /// The layer's geometry mask is not applied yet.
    GeometryMaskIgnored { layer: String },
    /// A fill (the layer's own, or content effect `effect`) projects from a volume and the compositor has no
    /// surface map for the set (no mesh, or the set's UV set is missing); it was left out instead of guessed.
    FillNeedsMesh { layer: String, effect: Option<usize>, mode: String },
    /// A fill's projection cannot be evaluated (a mode not evaluated yet, or one from a newer version); it was
    /// left out.
    ProjectionNotComputable { layer: String, effect: Option<usize>, reason: String },
}

impl Compositor<'_> {
    pub(super) fn collect_warnings(&self, layers: &[Layer], channel: Channel, depth: usize, out: &mut Vec<CompositeWarning>) {
        for l in layers.iter().filter(|l| affects(l, channel)) {
            let id = || l.id.clone();
            if l.images().iter().any(|(which, i)| reads(*which, channel) && i.size() != self.set.resolution) {
                out.push(CompositeWarning::ImageSizeMismatch { layer: id() });
            }
            let passthrough = l.blend_for(channel).is_some_and(|b| b.mode == BlendMode::Passthrough);
            match &l.kind {
                LayerKind::Fill(f) => match self.fill_problem(f, channel) {
                    Some(FillProblem::NoEvaluator) => out.push(CompositeWarning::MaterialFillSkipped { layer: id() }),
                    Some(FillProblem::Projection(p)) => out.push(projection_warning(id(), None, p)),
                    None => {}
                },
                LayerKind::Group(_) if depth >= MAX_GROUP_DEPTH => out.push(CompositeWarning::GroupTooDeep { layer: id() }),
                LayerKind::Group(children) => self.collect_warnings(children, channel, depth + 1, out),
                _ => {}
            }
            self.effect_warnings(l, channel, passthrough, out);
        }
    }

    fn effect_warnings(&self, l: &Layer, channel: Channel, passthrough: bool, out: &mut Vec<CompositeWarning>) {
        let applies = |e: &crate::paint::ContentEffect| e.enabled && e.blend_for(channel).is_some_and(|b| b.mode != BlendMode::Disable);
        let group = matches!(l.kind, LayerKind::Group(_));
        if group && passthrough && l.effects.iter().any(applies) {
            out.push(CompositeWarning::GroupEffectsSkipped { layer: l.id.clone() });
        } else {
            for (index, e) in l.effects.iter().enumerate().filter(|(_, e)| applies(e) && !self.content_effect_computable(e)) {
                let projection = match &e.kind {
                    EffectKind::Fill(f) => match self.fill_computable(f) {
                        Err(FillProblem::Projection(p)) => Some(p),
                        _ => None,
                    },
                    _ => None,
                };
                out.push(match projection {
                    Some(p) => projection_warning(l.id.clone(), Some(index), p),
                    None => CompositeWarning::EffectNotComputable { layer: l.id.clone(), in_mask: false, index, kind: e.kind.id().to_string() },
                });
            }
        }
        if let Some(mask) = l.mask.as_ref().filter(|m| m.enabled) {
            for (index, e) in mask.effects.iter().enumerate().filter(|(_, e)| e.enabled && !self.mask_effect_computable(e)) {
                out.push(CompositeWarning::EffectNotComputable { layer: l.id.clone(), in_mask: true, index, kind: e.kind.id().to_string() });
            }
        }
        if l.geometry_mask.is_some() {
            out.push(CompositeWarning::GeometryMaskIgnored { layer: l.id.clone() });
        }
    }
}

fn projection_warning(layer: String, effect: Option<usize>, p: NotComputable) -> CompositeWarning {
    match p {
        NotComputable::NeedsMesh { mode } => CompositeWarning::FillNeedsMesh { layer, effect, mode },
        other => CompositeWarning::ProjectionNotComputable { layer, effect, reason: other.to_string() },
    }
}

/// True if compositing `channel` reads the image.
fn reads(which: LayerImage, channel: Channel) -> bool {
    match which {
        LayerImage::Paint(c) | LayerImage::Effect { channel: c, .. } => c == channel,
        LayerImage::Mask { .. } => true,
    }
}
