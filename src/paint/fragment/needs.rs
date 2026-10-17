//! Which baked maps a fragment needs (AS-05): the maps its generators and filters read, as the caller's
//! lookup says (the paint model knows no concrete procedural), plus the maps colour selections pick from.

use super::FragmentContent;
use crate::paint::effect::{EffectDomain, EffectKind};
use crate::paint::layer::LayerWalk;
use crate::paint::texture_set::{BakedKind, TextureSet};

/// The baked maps `content` reads, in first-use order without repeats. `procedural_maps(id)` gives the maps the
/// generator or filter registered under `id` reads (nothing for an id it does not know). Switched-off effects
/// count too: switching one on must not need a bake first.
pub fn required_maps(content: &FragmentContent, procedural_maps: &dyn Fn(&str) -> Vec<BakedKind>) -> Vec<BakedKind> {
    let mut out = Vec::new();
    let mut visit = |maps: Vec<BakedKind>| {
        for m in maps {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    };
    match content {
        FragmentContent::Layers(layers) => {
            for (_, l) in LayerWalk::new(layers) {
                l.effects.iter().for_each(|e| visit(kind_maps(&e.kind, procedural_maps)));
                if let Some(m) = &l.mask {
                    m.effects.iter().for_each(|e| visit(kind_maps(&e.kind, procedural_maps)));
                }
            }
        }
        FragmentContent::Mask(mask) => mask.effects.iter().for_each(|e| visit(kind_maps(&e.kind, procedural_maps))),
    }
    out
}

fn kind_maps<D: EffectDomain>(kind: &EffectKind<D>, procedural_maps: &dyn Fn(&str) -> Vec<BakedKind>) -> Vec<BakedKind> {
    match kind {
        EffectKind::Generator(p) | EffectKind::Filter(p) => procedural_maps(&p.id),
        EffectKind::ColorSelection(c) => vec![c.map.clone()],
        _ => Vec::new(),
    }
}

/// The maps of `needs` that `set` has not baked.
pub fn missing(needs: &[BakedKind], set: &TextureSet) -> Vec<BakedKind> {
    needs.iter().filter(|k| set.baked(k).is_none()).cloned().collect()
}
