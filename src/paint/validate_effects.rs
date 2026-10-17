//! Invariants of a layer's effect stacks: opacities and the mask's initial value within 0..1, effect
//! channels declared and used once per effect, anchor ids non-empty and unique within the texture set.

use super::{DocError, check_channel_lists};
use crate::paint::effect::EffectKind;
use crate::paint::layer::Layer;
use crate::paint::texture_set::TextureSet;

/// Anchor ids seen so far in a texture set.
#[derive(Default)]
pub(super) struct Anchors(Vec<String>);

impl Anchors {
    /// False if `id` is empty or was seen before.
    fn claim(&mut self, id: &str) -> bool {
        let ok = !id.is_empty() && !self.0.iter().any(|a| a == id);
        self.0.push(id.to_string());
        ok
    }
}

pub(super) fn check_layer(set: &TextureSet, layer: &Layer, anchors: &mut Anchors, out: &mut Vec<DocError>) {
    let unit = |v: f32| (0.0..=1.0).contains(&v);
    let content_ok = layer.effects.iter().all(|e| e.blend.iter().all(|b| unit(b.opacity)));
    let mask_ok = layer.mask.as_ref().is_none_or(|m| unit(m.initial) && m.effects.iter().all(|e| unit(e.blend.opacity)));
    if !content_ok || !mask_ok {
        out.push(DocError::BadOpacity { set: set.id.clone(), layer: layer.id.clone() });
    }
    for e in &layer.effects {
        check_channel_lists(set, layer, &e.channel_lists(), out);
    }
    let content = layer.effects.iter().map(|e| &e.kind).filter_map(|k| match k {
        EffectKind::Anchor(a) => Some(&a.id),
        _ => None,
    });
    let mask = layer.mask.iter().flat_map(|m| &m.effects).map(|e| &e.kind).filter_map(|k| match k {
        EffectKind::Anchor(a) => Some(&a.id),
        _ => None,
    });
    for id in content.chain(mask) {
        if !anchors.claim(id) {
            out.push(DocError::BadAnchor { set: set.id.clone(), layer: layer.id.clone(), anchor: id.clone() });
        }
    }
}
