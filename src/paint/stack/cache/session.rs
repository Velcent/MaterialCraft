//! [`CacheSession`]: one build's view of a [`StackCache`], through which the compositor reads and keeps results.

use super::keys::Below;
use super::store::TileKey;
use super::{FILL, RESAMPLED, SEP, StackCache};
use crate::paint::channel::Channel;
use crate::paint::image::Rect;
use crate::paint::stack::fill_eval::FillTop;
use crate::tex::Tex;
use std::collections::HashMap;
use std::sync::Arc;

/// One build's view of a [`StackCache`]: the keys of what is kept for the set.
pub struct CacheSession<'c> {
    pub(super) cache: &'c StackCache,
    pub(super) set: String,
    /// Layer id → key of its mask's inputs, for the layers whose mask is kept.
    pub(super) keys: HashMap<String, u64>,
    /// Layer id → key of its fills, for the layers whose fills are kept.
    pub(super) fills: HashMap<String, u64>,
    /// Where the composite below a layer is kept (the layer edited last).
    pub(super) below: Option<Below>,
    /// Revision of the set's baked maps.
    pub(super) baked: u64,
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

    /// The baked map `kind` (its id) resampled to the set's resolution: kept, or `compute`d and kept (the few
    /// used last, a map is large).
    pub(crate) fn resampled(&self, kind: &str, compute: impl FnOnce() -> Arc<Tex>) -> Arc<Tex> {
        let found = self.cache.inner.lock().ok().and_then(|mut i| {
            let at = i.resampled.iter().position(|(s, k, rev, _)| *s == self.set && k == kind && *rev == self.baked)?;
            let entry = i.resampled.remove(at);
            let map = entry.3.clone();
            i.resampled.push(entry);
            Some(map)
        });
        if let Some(map) = found {
            return map;
        }
        let map = compute();
        if let Ok(mut i) = self.cache.inner.lock() {
            i.resampled.retain(|(s, k, _, _)| !(*s == self.set && k == kind));
            i.resampled.push((self.set.clone(), kind.to_string(), self.baked, map.clone()));
            let extra = i.resampled.len().saturating_sub(RESAMPLED);
            i.resampled.drain(..extra);
        }
        map
    }

    /// The key of `layer`'s fills if they are kept.
    pub(crate) fn fill_key(&self, layer: &str) -> Option<u64> {
        self.fills.get(layer).copied()
    }

    /// A fill of `layer` (its own for `slot` `None`, else content effect `slot`) for `channel` over `r` (under
    /// `key`): kept values, or `compute`d (kept when they vary per texel).
    pub(in crate::paint::stack) fn fill(
        &self,
        layer: &str,
        slot: Option<usize>,
        channel: Channel,
        key: u64,
        r: Rect,
        compute: impl FnOnce() -> Option<FillTop>,
    ) -> Option<FillTop> {
        let name = format!("{layer}{SEP}{FILL}{SEP}{}{SEP}{channel:?}", slot.map_or("base".to_string(), |s| s.to_string()));
        let k = TileKey::new(&self.set, &name, r);
        if let Some(v) = self.cache.inner.lock().ok().and_then(|mut i| i.fills.get(&k, key)) {
            // an empty entry: the fill writes nothing here
            return (!v.is_empty()).then(|| FillTop::Pixels(v.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect()));
        }
        let top = compute();
        let flat: Option<Vec<f32>> = match &top {
            Some(FillTop::Pixels(px)) => Some(px.iter().flatten().copied().collect()),
            Some(FillTop::Constant(_)) => None,
            None => Some(Vec::new()),
        };
        if let (Some(flat), Ok(mut i)) = (flat, self.cache.inner.lock()) {
            i.fills.put(k, key, &flat);
        }
        top
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
