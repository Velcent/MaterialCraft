//! The keys kept results are stored under: what an expensive mask or fill reads, and what the composite below the
//! layer edited last covers, as layer revisions ([`super::revisions`]).

use super::SEP;
use super::revisions::{SetRevs, hash};
use crate::paint::blend::BlendMode;
use crate::paint::channel::Channel;
use crate::paint::effect::EffectKind;
use crate::paint::layer::{FillSource, Layer, LayerKind, LayerWalk};
use crate::paint::texture_set::TextureSet;
use std::collections::HashMap;

/// How far references are followed for a mask's key (as far as the compositor follows them).
const REFERENCE_DEPTH: usize = 4;

/// The composite below the top-level layer edited last, kept so that edits of that layer (a slider drag) composite
/// only from it up. Its key covers every layer before it, the set's channels and baked maps and the context.
#[derive(Debug)]
pub(super) struct Below {
    pub(super) index: usize,
    pub(super) layer: String,
    pub(super) key: u64,
}

impl Below {
    pub(super) fn of(set: &TextureSet, revs: &SetRevs, context: u64) -> Option<Below> {
        let index = revs.first_changed.filter(|i| *i > 0)?;
        let layer = set.layers.get(index)?.id.clone();
        let prefix = revs.prefix.get(&layer).copied()?;
        let key = hash(&(context, revs.baked, prefix, set.resolution, format!("{:?}", set.channels), "below"));
        Some(Below { index, layer, key })
    }

    /// The store's layer name of the kept composite of `channel`.
    pub(super) fn entry(&self, channel: Channel) -> String {
        format!("{}{SEP}below{SEP}{channel:?}", self.layer)
    }

    pub(super) fn is_entry(name: &str, layer: &str) -> bool {
        name.strip_prefix(layer).is_some_and(|rest| rest.starts_with(&format!("{SEP}below{SEP}")))
    }
}

/// The keys of the layers whose fills vary over the texture: what a fill reads is its own layer (and the context).
pub(super) fn fill_keys(set: &TextureSet, revs: &SetRevs, context: u64) -> HashMap<String, u64> {
    // fills that cost more than reading an image: materials, procedural textures, projections through the mesh
    let varies = |f: &crate::paint::layer::Fill| matches!(f.source, FillSource::Material { .. } | FillSource::Texture(_)) || f.projection.mode.needs_mesh();
    let channels = format!("{:?}", set.channels);
    set.walk_layers()
        .filter(|(_, l)| matches!(&l.kind, LayerKind::Fill(f) if varies(f)) || l.effects.iter().any(|e| matches!(&e.kind, EffectKind::Fill(f) if varies(f))))
        .map(|(_, l)| (l.id.clone(), hash(&(context, revs.baked, revs.own.get(&l.id).copied().unwrap_or(0), set.resolution, &channels, "fill"))))
        .collect()
}

/// True for masks worth keeping: those with an effect that computes more than a texel at a time.
fn expensive(l: &Layer) -> bool {
    l.mask.as_ref().is_some_and(|m| m.effects.iter().any(|e| matches!(e.kind, EffectKind::Generator(_) | EffectKind::Filter(_) | EffectKind::Reference(_))))
}

/// The keys of the expensive masks of `set`.
pub(super) fn mask_keys(set: &TextureSet, revs: &SetRevs, context: u64) -> HashMap<String, u64> {
    let mut out = HashMap::new();
    for (_, l) in set.walk_layers().filter(|(_, l)| expensive(l)) {
        let mut parts = vec![context, revs.baked, hash(&set.resolution)];
        let mut seen = Vec::new();
        Reach { set, revs }.mask(l, REFERENCE_DEPTH, &mut seen, &mut parts);
        out.insert(l.id.clone(), hash(&parts));
    }
    out
}

/// What a mask or an anchor's stack reads, as revisions.
struct Reach<'s> {
    set: &'s TextureSet,
    revs: &'s SetRevs,
}

impl Reach<'_> {
    fn rev(map: &HashMap<String, u64>, id: &str) -> u64 {
        map.get(id).copied().unwrap_or(0)
    }

    /// `l`'s mask: the layer itself (the mask is part of it); with a generator also the stack before it and its
    /// content; through references what their anchors read, `depth` references deep.
    fn mask(&self, l: &Layer, depth: usize, seen: &mut Vec<String>, parts: &mut Vec<u64>) {
        parts.push(Self::rev(&self.revs.own, &l.id));
        let Some(mask) = &l.mask else { return };
        if mask.effects.iter().any(|e| matches!(e.kind, EffectKind::Generator(_))) {
            // generators read the height below the layer and its own content (see `StackContext`)
            parts.push(Self::rev(&self.revs.height_prefix, &l.id));
            parts.push(Self::rev(&self.revs.deep, &l.id));
        }
        for e in &mask.effects {
            if let EffectKind::Reference(r) = &e.kind {
                self.anchor(&r.anchor, depth, seen, parts);
            }
        }
    }

    /// What reading anchor `id` reads: the mask it sits in, or the content of its layer (with the stack below for an
    /// effect layer, whose content starts from it).
    fn anchor(&self, id: &str, depth: usize, seen: &mut Vec<String>, parts: &mut Vec<u64>) {
        if depth == 0 || seen.iter().any(|s| s == id) {
            parts.push(u64::MAX);
            return;
        }
        seen.push(id.to_string());
        match anchor_site(self.set, id) {
            None => parts.push(0),
            Some((a, true)) => self.mask(a, depth - 1, seen, parts),
            Some((a, false)) => {
                parts.push(Self::rev(&self.revs.deep, &a.id));
                let effect_layer = !matches!(a.kind, LayerKind::Group(_)) && a.blend.iter().any(|b| b.mode == BlendMode::Passthrough);
                if effect_layer {
                    parts.push(Self::rev(&self.revs.prefix, &a.id));
                }
                for e in &a.effects {
                    if let EffectKind::Reference(r) = &e.kind {
                        self.anchor(&r.anchor, depth - 1, seen, parts);
                    }
                }
            }
        }
    }
}

/// The layer holding anchor `id` and whether it sits in its mask, found as the compositor finds it.
fn anchor_site<'s>(set: &'s TextureSet, id: &str) -> Option<(&'s Layer, bool)> {
    for (_, l) in LayerWalk::new(&set.layers) {
        if l.effects.iter().any(|e| matches!(&e.kind, EffectKind::Anchor(a) if a.id == id)) {
            return Some((l, false));
        }
        if l.mask.as_ref().is_some_and(|m| m.effects.iter().any(|e| matches!(&e.kind, EffectKind::Anchor(a) if a.id == id))) {
            return Some((l, true));
        }
    }
    None
}
