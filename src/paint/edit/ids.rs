//! Fresh ids for new layers and anchors, and new ids for a copied layer tree (layer ids and anchor ids are
//! unique within a texture set; see [`crate::paint::validate`]).

use crate::paint::effect::{Anchor, Effect, EffectDomain, EffectKind};
use crate::paint::layer::{Layer, LayerKind};
use crate::paint::texture_set::TextureSet;
use std::collections::{HashMap, HashSet};

/// The layer and anchor ids a texture set uses, plus the ones handed out since. Ids are `<base><n>` with the
/// smallest free `n` from 1.
#[derive(Debug, Clone, Default)]
pub struct IdPool {
    layers: HashSet<String>,
    anchors: HashSet<String>,
}

impl IdPool {
    /// The ids `set` uses now.
    pub fn of(set: &TextureSet) -> IdPool {
        let mut pool = IdPool::default();
        for (_, l) in set.walk_layers() {
            pool.layers.insert(l.id.clone());
            for_each_anchor(l, &mut |a| {
                pool.anchors.insert(a.id.clone());
            });
        }
        pool
    }

    /// A layer id not used yet, reserved from now on.
    pub fn layer(&mut self, base: &str) -> String {
        fresh(&mut self.layers, base)
    }

    /// An anchor id not used yet, reserved from now on.
    pub fn anchor(&mut self, base: &str) -> String {
        fresh(&mut self.anchors, base)
    }

    /// Gives `layer` and everything in it (children, anchors) new ids, so the copy can be inserted next to the
    /// original. References inside the copy to anchors inside the copy follow them; references to anchors
    /// elsewhere are kept.
    pub fn reid(&mut self, layer: &mut Layer) {
        let mut renamed = HashMap::new();
        self.reid_tree(layer, &mut renamed);
        relink(layer, &renamed);
    }

    /// Gives the anchors among `effects` (a stack dropped onto a layer, e.g. a smart mask's) new ids; references
    /// among them follow, references to anchors elsewhere are kept.
    pub fn reid_effects<D: EffectDomain>(&mut self, effects: &mut [Effect<D>]) {
        let mut renamed = HashMap::new();
        for e in effects.iter_mut() {
            if let EffectKind::Anchor(a) = &mut e.kind {
                let new = self.anchor(&stem(&a.id));
                renamed.insert(std::mem::replace(&mut a.id, new.clone()), new);
            }
        }
        for e in effects {
            if let EffectKind::Reference(r) = &mut e.kind
                && let Some(new) = renamed.get(r.anchor.as_str())
            {
                r.anchor = new.clone();
            }
        }
    }

    fn reid_tree(&mut self, layer: &mut Layer, renamed: &mut HashMap<String, String>) {
        layer.id = self.layer(&stem(&layer.id));
        let mut rename = |a: &mut Anchor| {
            let new = self.anchor(&stem(&a.id));
            renamed.insert(std::mem::replace(&mut a.id, new.clone()), new);
        };
        for e in &mut layer.effects {
            if let EffectKind::Anchor(a) = &mut e.kind {
                rename(a);
            }
        }
        if let Some(m) = &mut layer.mask {
            for e in &mut m.effects {
                if let EffectKind::Anchor(a) = &mut e.kind {
                    rename(a);
                }
            }
        }
        if let LayerKind::Group(children) = &mut layer.kind {
            for c in children {
                self.reid_tree(c, renamed);
            }
        }
    }
}

/// `base` with a trailing number and separator removed (`fill_3` → `fill_`; `layer12` → `layer`), so copies
/// count up instead of growing their names.
fn stem(id: &str) -> String {
    let s = id.trim_end_matches(|c: char| c.is_ascii_digit());
    if s.is_empty() { "layer".to_string() } else { s.to_string() }
}

fn fresh(used: &mut HashSet<String>, base: &str) -> String {
    let base = if base.is_empty() { "id" } else { base };
    let mut n: u64 = 1;
    loop {
        let id = format!("{base}{n}");
        if !used.contains(&id) {
            used.insert(id.clone());
            return id;
        }
        n += 1;
    }
}

fn for_each_anchor(l: &Layer, f: &mut dyn FnMut(&Anchor)) {
    l.effects.iter().filter_map(|e| anchor_of(&e.kind)).for_each(&mut *f);
    if let Some(m) = &l.mask {
        m.effects.iter().filter_map(|e| anchor_of(&e.kind)).for_each(f);
    }
}

fn anchor_of<D: EffectDomain>(k: &EffectKind<D>) -> Option<&Anchor> {
    match k {
        EffectKind::Anchor(a) => Some(a),
        _ => None,
    }
}

fn relink(layer: &mut Layer, renamed: &HashMap<String, String>) {
    let follow = |anchor: &mut String| {
        if let Some(new) = renamed.get(anchor.as_str()) {
            *anchor = new.clone();
        }
    };
    for e in &mut layer.effects {
        if let EffectKind::Reference(r) = &mut e.kind {
            follow(&mut r.anchor);
        }
    }
    if let Some(m) = &mut layer.mask {
        for e in &mut m.effects {
            if let EffectKind::Reference(r) = &mut e.kind {
                follow(&mut r.anchor);
            }
        }
    }
    if let LayerKind::Group(children) = &mut layer.kind {
        for c in children {
            relink(c, renamed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::effect::{ContentEffect, Reference};
    use crate::paint::layer::PaintContent;

    fn anchor(id: &str) -> ContentEffect {
        ContentEffect::new(Vec::new(), EffectKind::Anchor(Anchor { id: id.into() }))
    }

    #[test]
    fn fresh_ids_skip_used_ones_and_copies_get_new_ids() {
        let mut set = TextureSet::new("s", "S", (8, 8));
        let inner = Layer::new("layer1", "A", LayerKind::Paint(PaintContent::default()))
            .with_effect(anchor("dirt1"))
            .with_effect(ContentEffect::new(Vec::new(), EffectKind::Reference(Reference { anchor: "dirt1".into() })))
            .with_effect(ContentEffect::new(Vec::new(), EffectKind::Reference(Reference { anchor: "outside".into() })));
        set.layers = vec![Layer::new("group1", "G", LayerKind::Group(vec![inner])), Layer::new("layer3", "B", LayerKind::Group(vec![]))];
        let mut pool = IdPool::of(&set);
        assert_eq!(pool.layer("layer"), "layer2");
        assert_eq!(pool.layer("layer"), "layer4");
        assert_eq!(pool.anchor("dirt"), "dirt2");
        let mut copy = set.layers[0].clone();
        pool.reid(&mut copy);
        assert_eq!(copy.id, "group2");
        let child = &copy.children()[0];
        assert_eq!(child.id, "layer5");
        let kinds: Vec<String> = child
            .effects
            .iter()
            .map(|e| match &e.kind {
                EffectKind::Anchor(a) => a.id.clone(),
                EffectKind::Reference(r) => r.anchor.clone(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(kinds, ["dirt3", "dirt3", "outside"]);
        assert_eq!(stem("fill_3"), "fill_");
        assert_eq!(stem("42"), "layer");
    }
}
