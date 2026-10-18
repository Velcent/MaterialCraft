//! How far a change of some layers spreads through a texture set's stack: the composite can differ only within
//! that many texels of the texels that changed. A brush stroke's tiles grown by it are composited again on their
//! own ([`crate::paint::MaterialMaps::tiles_reached`]); the rest of the maps stay as they are.
//!
//! The bound adds up the reaches ([`crate::paint::effect::EffectReach`]) of every effect that may read a changed
//! layer: the layer's own effects and those of the folders holding it; effects of layers above it that read the
//! stack (effect layers, generators reading the heights around their layer, references); effects of layers below
//! it that read through references. An effect that reads the whole image makes the whole set the reach.
//!
//! Tiles are composited again over the same rectangles as the whole build (so the same values, even where a
//! filter's result depends on the rectangle, such as island padding); the reach only has to bound where the
//! values can change, which the effects' reaches do.

use super::Compositor;
use crate::paint::blend::BlendMode;
use crate::paint::effect::{EffectDomain, EffectKind, EvalTarget, Reach};
use crate::paint::layer::{Layer, LayerKind};

impl Compositor<'_> {
    /// How far (texels) a change of the images of the layers `changed` (ids) spreads into the composite; `None`
    /// for the whole set (an effect that reads everything, or a layer not in the set).
    pub fn reach(&self, changed: &[&str]) -> Option<u32> {
        let order: Vec<(&Layer, Vec<&str>)> = walk_with_ancestors(&self.set.layers);
        let mut first = None;
        for id in changed {
            let at = order.iter().position(|(l, _)| l.id == *id)?;
            first = Some(first.map_or(at, |f: usize| f.min(at)));
        }
        let first = first?;
        let mut total = Reach::Texel;
        for (i, (l, ancestors)) in order.iter().enumerate() {
            let own = changed.contains(&l.id.as_str());
            let holds = order.iter().any(|(c, anc)| changed.contains(&c.id.as_str()) && anc.contains(&l.id.as_str()));
            let inside = ancestors.iter().any(|a| changed.contains(a));
            let reads = if i > first { self.reads_around(l) } else { has_reference(l) };
            if own || holds || inside || reads {
                total = total.then(self.layer_reach(l));
            }
        }
        total.texels()
    }

    /// The reaches of a layer's content and mask effects added up (one may read the output of another).
    fn layer_reach(&self, l: &Layer) -> Reach {
        let mut total = Reach::Texel;
        for e in &l.effects {
            let widest = self.set.channels.iter().map(|s| self.effect_reach(e, EvalTarget::Channel(s.channel))).fold(Reach::Texel, Reach::union);
            total = total.then(widest);
        }
        for e in l.mask.iter().flat_map(|m| m.effects.iter()) {
            total = total.then(self.effect_reach(e, EvalTarget::Mask));
        }
        total
    }

    /// How far one effect reads: around its input, and around its layer in the stack's heights.
    fn effect_reach<D: EffectDomain>(&self, e: &crate::paint::effect::Effect<D>, target: EvalTarget) -> Reach {
        let r = e.reach(self.effects, self.set, target);
        r.input.then(r.stack_height.unwrap_or(Reach::Texel))
    }

    /// True for a generator that reads the stack around its layer.
    fn reads_stack<D: EffectDomain>(&self, k: &EffectKind<D>, target: EvalTarget) -> bool {
        matches!(k, EffectKind::Generator(_)) && k.reach(self.effects, self.set, target).stack_height.is_some()
    }

    /// True if a layer above a changed one may read it: an effect layer (its content starts from the stack below),
    /// a generator reading the stack around its layer, or a reference.
    fn reads_around(&self, l: &Layer) -> bool {
        let effect_layer = !matches!(l.kind, LayerKind::Group(_)) && l.blend.iter().any(|b| b.mode == BlendMode::Passthrough);
        let content = l.effects.iter().any(|e| e.enabled && self.reads_stack(&e.kind, EvalTarget::Channel(crate::paint::Channel::Height)));
        let mask = l.mask.iter().flat_map(|m| m.effects.iter()).any(|e| e.enabled && self.reads_stack(&e.kind, EvalTarget::Mask));
        effect_layer || content || mask || has_reference(l)
    }
}

/// Every layer in compositing walk order (folders before their children) with the ids of the folders holding it.
fn walk_with_ancestors(layers: &[Layer]) -> Vec<(&Layer, Vec<&str>)> {
    fn go<'a>(layers: &'a [Layer], path: &mut Vec<&'a str>, out: &mut Vec<(&'a Layer, Vec<&'a str>)>) {
        for l in layers {
            out.push((l, path.clone()));
            if let LayerKind::Group(children) = &l.kind {
                path.push(&l.id);
                go(children, path, out);
                path.pop();
            }
        }
    }
    let mut out = Vec::new();
    go(layers, &mut Vec::new(), &mut out);
    out
}

/// True if a layer has an enabled reference (it may read a layer anywhere).
fn has_reference(l: &Layer) -> bool {
    let is = |enabled: bool, k: bool| enabled && k;
    l.effects.iter().any(|e| is(e.enabled, matches!(e.kind, EffectKind::Reference(_))))
        || l.mask.iter().flat_map(|m| m.effects.iter()).any(|e| is(e.enabled, matches!(e.kind, EffectKind::Reference(_))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::effect::{Procedural, Reference};
    use crate::paint::test_stacks::Blur;
    use crate::paint::{Channel, ContentEffect, EffectRegistry, Fill, PaintContent, TextureSet};

    fn layer(id: &str, mode: BlendMode) -> Layer {
        Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(Channel::BaseColor, [0.5; 4])]))).with_blend(Channel::BaseColor, mode, 1.0)
    }

    fn with(l: Layer, kind: EffectKind<crate::paint::effect::Content>) -> Layer {
        l.with_effect(ContentEffect::new(Vec::new(), kind).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0))
    }

    #[test]
    fn filters_reading_the_changed_layer_add_up() {
        let mut reg = EffectRegistry::new();
        reg.add_filter("r3", Box::new(Blur(3)));
        reg.add_filter("r5", Box::new(Blur(5)));
        let f = |id: &str| EffectKind::Filter(Procedural::new(id));
        let mut s = TextureSet::new("s", "S", (64, 64)).with_channel(Channel::BaseColor);
        s.layers.push(with(layer("below", BlendMode::Normal), f("r5"))); // reads only itself
        s.layers.push(Layer::new("p", "P", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
        s.layers.push(with(layer("plain", BlendMode::Normal), f("r5"))); // above, but reads only itself
        s.layers.push(with(layer("fx", BlendMode::Passthrough), f("r3"))); // reads the stack below
        let folder = Layer::new("g", "G", LayerKind::Group(vec![with(layer("inner", BlendMode::Passthrough), f("r5"))]));
        s.layers.push(folder.with_blend(Channel::BaseColor, BlendMode::Normal, 1.0)); // an isolated folder: its effect layer reads only inside
        let c = Compositor::with_effects(&s, &reg);
        assert_eq!(c.reach(&["p"]), Some(3 + 5), "the effect layers above: one at top level, one inside a folder (conservatively)");
        assert_eq!(c.reach(&["below"]), Some(5 + 3 + 5));
        assert_eq!(c.reach(&["nope"]), None);
        // a reference below the changed layer counts too
        s.layers.insert(0, with(layer("refers", BlendMode::Normal), EffectKind::Reference(Reference { anchor: "x".into(), channel: None, levels: None })));
        s.layers[0] = with(s.layers[0].clone(), f("r3"));
        let c = Compositor::with_effects(&s, &reg);
        assert_eq!(c.reach(&["p"]), Some(3 + 3 + 5));
    }
}
