//! [`TileStore`]: cached grey values of masks over tiles, each entry under the key of what it was computed from,
//! within a byte budget (least recently used entries go first). Uniform tiles (a sparse mask's empty ones) are kept
//! as one value.

use crate::paint::image::Rect;
use std::collections::HashMap;
use std::sync::Arc;

/// One layer's mask over one rectangle of one texture set.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TileKey {
    pub set: String,
    pub layer: String,
    pub rect: (u32, u32, u32, u32),
}

impl TileKey {
    pub fn new(set: &str, layer: &str, r: Rect) -> TileKey {
        TileKey { set: set.to_string(), layer: layer.to_string(), rect: (r.x, r.y, r.w, r.h) }
    }
}

#[derive(Debug, Clone)]
enum Values {
    /// Every texel the same (by bits).
    Uniform(f32, usize),
    Each(Arc<[f32]>),
}

impl Values {
    fn of(v: &[f32]) -> Values {
        match v.first() {
            Some(first) if v.iter().all(|x| x.to_bits() == first.to_bits()) => Values::Uniform(*first, v.len()),
            _ => Values::Each(Arc::from(v)),
        }
    }

    fn to_vec(&self) -> Vec<f32> {
        match self {
            Values::Uniform(v, n) => vec![*v; *n],
            Values::Each(v) => v.to_vec(),
        }
    }

    fn bytes(&self) -> usize {
        let values = match self {
            Values::Uniform(..) => 0,
            Values::Each(v) => v.len() * 4,
        };
        values + 96
    }
}

struct Entry {
    dep: u64,
    values: Values,
    used: u64,
}

pub(super) struct TileStore {
    entries: HashMap<TileKey, Entry>,
    bytes: usize,
    budget: usize,
    tick: u64,
}

impl TileStore {
    pub fn new(budget: usize) -> TileStore {
        TileStore { entries: HashMap::new(), bytes: 0, budget, tick: 0 }
    }

    /// The values under `key` if they were computed from `dep`.
    pub fn get(&mut self, key: &TileKey, dep: u64) -> Option<Vec<f32>> {
        self.tick += 1;
        let e = self.entries.get_mut(key).filter(|e| e.dep == dep)?;
        e.used = self.tick;
        Some(e.values.to_vec())
    }

    pub fn put(&mut self, key: TileKey, dep: u64, values: &[f32]) {
        self.tick += 1;
        let values = Values::of(values);
        self.bytes += values.bytes();
        if let Some(old) = self.entries.insert(key, Entry { dep, values, used: self.tick }) {
            self.bytes -= old.values.bytes();
        }
        if self.bytes > self.budget {
            self.evict(self.budget / 4 * 3);
        }
    }

    /// Drops the least recently used entries until at most `target` bytes are kept.
    fn evict(&mut self, target: usize) {
        let mut ages: Vec<(u64, TileKey)> = self.entries.iter().map(|(k, e)| (e.used, k.clone())).collect();
        ages.sort_unstable_by_key(|(used, _)| *used);
        for (_, k) in ages {
            if self.bytes <= target {
                break;
            }
            if let Some(e) = self.entries.remove(&k) {
                self.bytes -= e.values.bytes();
            }
        }
    }

    /// Keeps the entries `keep` gives a key for, under that key; drops the others.
    pub fn retain_map(&mut self, keep: impl Fn(&TileKey, u64) -> Option<u64>) {
        let mut freed = 0;
        self.entries.retain(|k, e| match keep(k, e.dep) {
            Some(dep) => {
                e.dep = dep;
                true
            }
            None => {
                freed += e.values.bytes();
                false
            }
        });
        self.bytes -= freed;
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(layer: &str, x: u32) -> TileKey {
        TileKey::new("s", layer, Rect { x, y: 0, w: 4, h: 4 })
    }

    #[test]
    fn entries_answer_only_their_dep_and_old_ones_are_evicted() {
        let mut s = TileStore::new(3 * (64 + 96));
        s.put(key("a", 0), 1, &[0.5; 16]);
        assert_eq!(s.get(&key("a", 0), 1), Some(vec![0.5; 16]));
        assert_eq!(s.get(&key("a", 0), 2), None);
        let ramp: Vec<f32> = (0..16).map(|i| i as f32).collect();
        for x in 1..6 {
            s.put(key("b", x), 1, &ramp);
        }
        assert!(s.bytes() <= 3 * (64 + 96));
        assert_eq!(s.get(&key("b", 5), 1), Some(ramp));
        assert_eq!(s.get(&key("b", 1), 1), None, "the oldest went first");
        s.retain_map(|k, dep| (k.layer != "b").then_some(dep));
        assert_eq!(s.get(&key("b", 5), 1), None);
    }
}
