//! Whether a texture set's stack is local: every texel of the composite depends only on the layers' values at that
//! texel. Then a change confined to some tiles (a brush stroke) changes the composite only in those tiles, and they
//! can be composited again on their own. Filters read neighbouring texels, generators may read the stack around a
//! texel, and references read other stacks through them, so any of them makes the stack non-local.

use super::Compositor;
use crate::paint::effect::EffectKind;
use crate::paint::layer::Layer;

impl Compositor<'_> {
    /// True if the set's stack is local (see the module docs); conservative: false whenever it can't tell.
    pub fn is_local(&self) -> bool {
        layers_local(&self.set.layers)
    }
}

fn layers_local(layers: &[Layer]) -> bool {
    layers.iter().all(|l| {
        let content = l.effects.iter().all(|e| kind_local(&e.kind));
        let mask = l.mask.as_ref().is_none_or(|m| m.effects.iter().all(|e| kind_local(&e.kind)));
        content && mask && layers_local(l.children())
    })
}

fn kind_local<D: crate::paint::effect::EffectDomain>(k: &EffectKind<D>) -> bool {
    !matches!(k, EffectKind::Filter(_) | EffectKind::Generator(_) | EffectKind::Reference(_))
}

#[cfg(test)]
mod tests {
    use crate::paint::effect::{Procedural, Reference};
    use crate::paint::{BlendMode, Channel, Compositor, ContentEffect, EffectKind, Fill, Layer, LayerKind, TextureSet};

    fn fill_layer(id: &str) -> Layer {
        Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(Channel::BaseColor, [0.2; 4])]))).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0)
    }

    #[test]
    fn filters_generators_and_references_are_not_local() {
        let mut set = TextureSet::new("s", "S", (8, 8));
        set.layers.push(fill_layer("a"));
        assert!(Compositor::new(&set).is_local());
        let kinds: [EffectKind<_>; 3] = [
            EffectKind::Filter(Procedural::new("blur")),
            EffectKind::Generator(Procedural::new("dirt")),
            EffectKind::Reference(Reference { anchor: "x".into(), channel: None, levels: None }),
        ];
        for kind in kinds {
            let mut nested = set.clone();
            let mut inner = fill_layer("b");
            inner.effects.push(ContentEffect::new(Vec::new(), kind));
            nested.layers.push(Layer::new("g", "G", LayerKind::Group(vec![inner])));
            assert!(!Compositor::new(&nested).is_local());
        }
    }
}
