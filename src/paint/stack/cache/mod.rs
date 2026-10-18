//! [`StackCache`]: results kept across builds of a texture set's maps, so that an edit or a brush stroke recomputes
//! only what depends on it: expensive masks, expensive fills (materials, procedural textures, fills projected
//! through the mesh), the composite below the top-level layer edited last, and baked maps resampled to the set's
//! resolution. On big imported projects most compositing time goes into masks with generators, filters and
//! references (AS: two thirds), and a mask is evaluated again for every channel; an edit of one layer leaves almost
//! all of them as they were.
//!
//! A mask's tiles are kept under a key of everything the mask reads ([`CacheSession::mask_key`]): the layer itself
//! (its mask is part of it), the caller's context (surface maps, the mesh) and the set's baked maps; a mask with a
//! generator also reads the stack below its layer and its layer's content, a reference the stack it points into.
//! Those are covered by layer revisions ([`revisions`]): copies of the layers as last seen are compared with the
//! layers of each build, so any change, however it was made (an edit, an undo, a stroke), gives new keys. Masks
//! made of paint, fills and levels only are cheap and not kept, and so are fills of images read through the UVs
//! (keeping them crowded out the masks: AS stroke frames 2.5 times slower).
//!
//! The cache decides which kept tiles are still valid inside one set. Which sets need compositing at all after an
//! undo or a redo is decided before any build, by [`crate::paint::set_changes::CompositeState`].
//!
//! A cached mask is the very values a computation gives, so a build with the cache has the same bits as one
//! without. Only masks evaluated directly for their layer are cached (not those read through references, which
//! may be cut off at a depth).

mod keys;
mod revisions;
mod session;
mod store;

use crate::paint::image::Rect;
use crate::paint::texture_set::TextureSet;
use crate::tex::Tex;
use keys::{Below, fill_keys, mask_keys};
use revisions::Revisions;
pub use session::CacheSession;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use store::{TileKey, TileStore};

/// Separates a layer id from what of it an entry holds (ids hold no control characters).
const SEP: char = '\u{1}';
/// Resampled baked maps kept.
const RESAMPLED: usize = 4;
/// The part name of kept fills.
const FILL: &str = "fill";

/// Bytes of mask and composite tiles a [`StackCache`] keeps by default (fills get half as much again).
pub const DEFAULT_BUDGET: usize = 1 << 30;

/// Results kept for the texture sets composited with it (see the module docs). Shared between threads.
pub struct StackCache {
    inner: Mutex<Inner>,
}

struct Inner {
    sets: HashMap<String, SetState>,
    store: TileStore,
    /// Fills apart, so many large fills can't push out the masks.
    fills: TileStore,
    /// Baked maps resampled to their set's resolution: (set, map, baked revision, map), the latest last.
    resampled: Vec<(String, String, u64, Arc<Tex>)>,
}

/// What the cache knows of one texture set: the context and layer revisions of its last build and the keys its
/// kept results had then.
struct SetState {
    context: u64,
    revisions: Revisions,
    keys: Keys,
}

/// The keys of a build's kept results.
#[derive(Default, Clone)]
struct Keys {
    masks: HashMap<String, u64>,
    /// Layer id → key of its fills (those that vary over the texture).
    fills: HashMap<String, u64>,
    below: Option<(String, u64)>,
}

impl Keys {
    /// The key a kept result stored under `name` has in this build.
    fn of(&self, name: &str) -> Option<u64> {
        if let Some(k) = self.masks.get(name) {
            return Some(*k);
        }
        if let Some((layer, FILL)) = name.split_once(SEP).map(|(l, rest)| (l, rest.split(SEP).next().unwrap_or(""))) {
            return self.fills.get(layer).copied();
        }
        self.below.as_ref().filter(|(layer, _)| Below::is_entry(name, layer)).map(|(_, k)| *k)
    }
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
    /// A cache keeping at most `budget` bytes of mask and composite tiles and half of that of fill tiles.
    pub fn new(budget: usize) -> StackCache {
        StackCache {
            inner: Mutex::new(Inner { sets: HashMap::new(), store: TileStore::new(budget), fills: TileStore::new(budget / 2), resampled: Vec::new() }),
        }
    }

    /// Starts a build of `set`. `context` stands for what the caller's fill context gives the masks (surface maps,
    /// the mesh): a different value drops what was kept for the set. Results whose inputs changed since the last
    /// build of the set are dropped, except, when the change is `confined` to an area (a brush stroke's reach, see
    /// [`crate::paint::MaterialMaps::tiles_reached`]), those of rectangles outside it: they are the same and are kept
    /// under their new keys.
    pub fn session(&self, set: &TextureSet, context: u64, confined: Option<&[Rect]>) -> CacheSession<'_> {
        let mut keys = Keys::default();
        let mut below = None;
        let mut baked = 0;
        if let Ok(mut inner) = self.inner.lock() {
            let Inner { sets, store, fills, .. } = &mut *inner;
            let state = sets.entry(set.id.clone()).or_insert_with(|| SetState { context, revisions: Revisions::default(), keys: Keys::default() });
            if state.context != context {
                *state = SetState { context, revisions: Revisions::default(), keys: Keys::default() };
            }
            if let Some(revs) = state.revisions.observe(set) {
                keys.masks = mask_keys(set, &revs, context);
                keys.fills = fill_keys(set, &revs, context);
                below = Below::of(set, &revs, context);
                baked = revs.baked;
            }
            keys.below = below.as_ref().map(|b| (b.layer.clone(), b.key));
            let old = std::mem::replace(&mut state.keys, keys.clone());
            let keep = |k: &TileKey, dep: u64| -> Option<u64> {
                if k.set != set.id {
                    return Some(dep);
                }
                let new = keys.of(&k.layer)?;
                if new == dep {
                    return Some(dep);
                }
                let outside = confined.is_some_and(|area| !area.iter().any(|a| overlaps(*a, k.rect)));
                (outside && old.of(&k.layer) == Some(dep)).then_some(new)
            };
            store.retain_map(keep);
            fills.retain_map(keep);
        }
        CacheSession { cache: self, set: set.id.clone(), keys: keys.masks, fills: keys.fills, below, baked }
    }

    /// Bytes of tiles kept.
    pub fn bytes(&self) -> usize {
        self.inner.lock().map(|i| i.store.bytes() + i.fills.bytes()).unwrap_or(0)
    }
}

fn overlaps(a: Rect, (x, y, w, h): (u32, u32, u32, u32)) -> bool {
    a.x < x + w && x < a.x + a.w && a.y < y + h && y < a.y + a.h
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
