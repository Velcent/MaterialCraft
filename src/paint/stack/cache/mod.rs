//! [`StackCache`]: results of expensive masks kept across builds of a texture set's maps, so that an edit
//! recomputes only what depends on it. On big imported projects most compositing time goes into masks with
//! generators, filters and references (AS: two thirds), and a mask is evaluated again for every channel; an edit
//! of one layer leaves almost all of them as they were.
//!
//! A mask's tiles are kept under a key of everything the mask reads ([`CacheSession::mask_key`]): the layer itself
//! (its mask is part of it), the caller's context (surface maps, the mesh) and the set's baked maps; a mask with a
//! generator also reads the stack below its layer and its layer's content, a reference the stack it points into.
//! Those are covered by layer revisions ([`revisions`]): copies of the layers as last seen are compared with the
//! layers of each build, so any change, however it was made (an edit, an undo, a stroke), gives new keys. Masks
//! made of paint, fills and levels only are cheap and not kept.
//!
//! A cached mask is the very values a computation gives, so a build with the cache has the same bits as one
//! without. Only masks evaluated directly for their layer are cached (not those read through references, which
//! may be cut off at a depth).

mod revisions;
mod store;

use crate::paint::blend::BlendMode;
use crate::paint::channel::Channel;
use crate::paint::effect::EffectKind;
use crate::paint::image::Rect;
use crate::paint::layer::{Layer, LayerKind, LayerWalk};
use crate::paint::texture_set::TextureSet;
use revisions::{Revisions, SetRevs, hash};
use std::collections::HashMap;
use std::sync::Mutex;
use store::{TileKey, TileStore};

/// Bytes of mask tiles a [`StackCache`] keeps by default.
pub const DEFAULT_BUDGET: usize = 1 << 30;
/// How far references are followed for a mask's key (as far as the compositor follows them).
const REFERENCE_DEPTH: usize = 4;

/// Mask results of the texture sets composited with it (see the module docs). Shared between threads.
pub struct StackCache {
    inner: Mutex<Inner>,
}

struct Inner {
    sets: HashMap<String, (u64, Revisions)>,
    store: TileStore,
}

impl Default for StackCache {
    fn default() -> StackCache {
        StackCache::new(DEFAULT_BUDGET)
    }
}

impl std::fmt::Debug for StackCache {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "StackCache({} bytes)", self.bytes())
    }
}

impl StackCache {
    /// A cache keeping at most `budget` bytes of mask tiles.
    pub fn new(budget: usize) -> StackCache {
        StackCache { inner: Mutex::new(Inner { sets: HashMap::new(), store: TileStore::new(budget) }) }
    }

    /// Starts a build of `set`. `context` stands for what the caller's fill context gives the masks (surface maps,
    /// the mesh): a different value drops what was kept for the set. Masks whose inputs changed since the last
    /// build of the set are dropped.
    pub fn session(&self, set: &TextureSet, context: u64) -> CacheSession<'_> {
        let mut keys = HashMap::new();
        let mut below = None;
        if let Ok(mut inner) = self.inner.lock() {
            let entry = inner.sets.entry(set.id.clone()).or_insert_with(|| (context, Revisions::default()));
            if entry.0 != context {
                *entry = (context, Revisions::default());
            }
            if let Some(revs) = entry.1.observe(set) {
                keys = mask_keys(set, &revs, context);
                below = Below::of(set, &revs, context);
            }
            let kept_below = below.as_ref().map(|b| (b.layer.clone(), b.key));
            inner.store.retain(|k, dep| {
                k.set != set.id || keys.get(&k.layer) == Some(&dep) || kept_below.as_ref().is_some_and(|(l, key)| Below::is_entry(&k.layer, l) && *key == dep)
            });
        }
        CacheSession { cache: self, set: set.id.clone(), keys, below }
    }

    /// Bytes of mask tiles kept.
    pub fn bytes(&self) -> usize {
        self.inner.lock().map(|i| i.store.bytes()).unwrap_or(0)
    }
}

/// One build's view of a [`StackCache`]: the keys of the set's expensive masks.
pub struct CacheSession<'c> {
    cache: &'c StackCache,
    set: String,
    /// Layer id → key of its mask's inputs, for the layers whose mask is kept.
    keys: HashMap<String, u64>,
    /// Where the composite below a layer is kept (the layer edited last).
    below: Option<Below>,
}

impl std::fmt::Debug for CacheSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "CacheSession({}, {} masks)", self.set, self.keys.len())
    }
}

impl CacheSession<'_> {
    /// The key of `layer`'s mask inputs if its mask is kept.
    pub(crate) fn mask_key(&self, layer: &str) -> Option<u64> {
        self.keys.get(layer).copied()
    }

    /// The mask of `layer` over `r` (under `key`): kept values, or `compute`d and kept.
    pub(crate) fn mask(&self, layer: &str, key: u64, r: Rect, compute: impl FnOnce() -> Vec<f32>) -> Vec<f32> {
        let k = TileKey::new(&self.set, layer, r);
        if let Some(v) = self.cache.inner.lock().ok().and_then(|mut i| i.store.get(&k, key)) {
            return v;
        }
        let v = compute();
        if let Ok(mut i) = self.cache.inner.lock() {
            i.store.put(k, key, &v);
        }
        v
    }

    /// Where compositing `channel` over `r` can start: the index of the top-level layer the kept composite below it
    /// is for, and that composite (working space, as the compositor has it there).
    pub(crate) fn start(&self, channel: Channel, r: Rect) -> Option<(usize, Vec<[f32; 4]>)> {
        let b = self.below.as_ref()?;
        let k = TileKey::new(&self.set, &b.entry(channel), r);
        let flat = self.cache.inner.lock().ok().and_then(|mut i| i.store.get(&k, b.key))?;
        Some((b.index, flat.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()))
    }

    /// The top-level layer before which the composite is kept, if any.
    pub(crate) fn keeps_below(&self) -> Option<usize> {
        self.below.as_ref().map(|b| b.index)
    }

    /// Keeps the composite of `channel` over `r` below the layer of [`Self::keeps_below`].
    pub(crate) fn keep_below(&self, channel: Channel, r: Rect, acc: &[[f32; 4]]) {
        let Some(b) = &self.below else { return };
        let flat: Vec<f32> = acc.iter().flatten().copied().collect();
        if let Ok(mut i) = self.cache.inner.lock() {
            i.store.put(TileKey::new(&self.set, &b.entry(channel), r), b.key, &flat);
        }
    }
}

/// The composite below the top-level layer edited last, kept so that edits of that layer (a slider drag) composite
/// only from it up. Its key covers every layer before it, the set's channels and baked maps and the context.
#[derive(Debug)]
struct Below {
    index: usize,
    layer: String,
    key: u64,
}

impl Below {
    fn of(set: &TextureSet, revs: &SetRevs, context: u64) -> Option<Below> {
        let index = revs.first_changed.filter(|i| *i > 0)?;
        let layer = set.layers.get(index)?.id.clone();
        let prefix = revs.prefix.get(&layer).copied()?;
        let key = hash(&(context, revs.baked, prefix, set.resolution, format!("{:?}", set.channels), "below"));
        Some(Below { index, layer, key })
    }

    /// The store's layer name of the kept composite of `channel`.
    fn entry(&self, channel: Channel) -> String {
        format!("{}\u{1}below\u{1}{channel:?}", self.layer)
    }

    fn is_entry(name: &str, layer: &str) -> bool {
        name.strip_prefix(layer).is_some_and(|rest| rest.starts_with("\u{1}below\u{1}"))
    }
}

/// True for masks worth keeping: those with an effect that computes more than a texel at a time.
fn expensive(l: &Layer) -> bool {
    l.mask.as_ref().is_some_and(|m| m.effects.iter().any(|e| matches!(e.kind, EffectKind::Generator(_) | EffectKind::Filter(_) | EffectKind::Reference(_))))
}

/// The keys of the expensive masks of `set`.
fn mask_keys(set: &TextureSet, revs: &SetRevs, context: u64) -> HashMap<String, u64> {
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
