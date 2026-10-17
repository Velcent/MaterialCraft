//! [`MaterialCache`]: renders by [`MaterialKey`] (graph text, overrides, size), shared by every compositing run
//! and thread. A render happens once per key: threads asking for a key being rendered wait for it. Least
//! recently used renders are dropped once the cache holds more than its byte budget; failures are kept too, so
//! a broken graph is not evaluated again for every tile.

use super::material::{Material, MaterialError, MaterialKey};
use super::render::RenderedMaterial;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

/// Default budget: 1 GiB (about five 2K materials with six channels each).
pub const DEFAULT_BUDGET: usize = 1 << 30;

type Rendered = Result<Arc<RenderedMaterial>, MaterialError>;

struct Slot {
    cell: Arc<OnceLock<Rendered>>,
    used: u64,
}

impl Slot {
    fn bytes(&self) -> usize {
        match self.cell.get() {
            Some(Ok(r)) => r.bytes(),
            _ => 0,
        }
    }
}

#[derive(Default)]
struct Inner {
    slots: HashMap<MaterialKey, Slot>,
    tick: u64,
}

/// Rendered materials by key.
pub struct MaterialCache {
    budget: usize,
    inner: Mutex<Inner>,
}

impl Default for MaterialCache {
    fn default() -> MaterialCache {
        MaterialCache::new(DEFAULT_BUDGET)
    }
}

impl MaterialCache {
    /// A cache that keeps at most `budget` bytes of renders (the one in use is always kept).
    pub fn new(budget: usize) -> MaterialCache {
        MaterialCache { budget, inner: Mutex::new(Inner::default()) }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The render of `material` at `size`, rendered now if it is not cached.
    pub fn get_or_render(&self, material: &Material, size: usize) -> Rendered {
        let key = material.key(size);
        let cell = {
            let mut inner = self.lock();
            inner.tick += 1;
            let tick = inner.tick;
            let slot = inner.slots.entry(key).or_insert_with(|| Slot { cell: Arc::new(OnceLock::new()), used: tick });
            slot.used = tick;
            slot.cell.clone()
        };
        let fresh = cell.get().is_none();
        let out = cell.get_or_init(|| RenderedMaterial::render(material, size).map(Arc::new)).clone();
        if fresh {
            self.evict(key);
        }
        out
    }

    /// The cached render under `key`, if it is done (no render is started).
    pub fn get(&self, key: MaterialKey) -> Option<Rendered> {
        self.lock().slots.get(&key).and_then(|s| s.cell.get().cloned())
    }

    /// Number of keys held (done, failed or being rendered).
    pub fn len(&self) -> usize {
        self.lock().slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bytes held by finished renders.
    pub fn bytes(&self) -> usize {
        self.lock().slots.values().map(Slot::bytes).sum()
    }

    pub fn clear(&self) {
        self.lock().slots.clear();
    }

    /// Drops least recently used finished renders other than `keep` until the budget holds.
    fn evict(&self, keep: MaterialKey) {
        let mut inner = self.lock();
        let mut total: usize = inner.slots.values().map(Slot::bytes).sum();
        while total > self.budget {
            let oldest = inner.slots.iter().filter(|(k, s)| **k != keep && s.cell.get().is_some()).min_by_key(|(_, s)| s.used).map(|(k, s)| (*k, s.bytes()));
            let Some((key, bytes)) = oldest else { break };
            inner.slots.remove(&key);
            total = total.saturating_sub(bytes);
        }
    }
}
