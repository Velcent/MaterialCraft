//! Caches shared by all generators, so a generator's whole-texture result is computed once and every tile and
//! channel pass of the compositor only copies from it:
//!
//! - [`set_maps`]: the [`SetMaps`] of a texture set, by a fingerprint of its baked maps;
//! - [`field`]: one generator's result at the set's resolution (16 bits per texel), by set fingerprint,
//!   generator id and parameters. Least recently used results are dropped above [`FIELD_BUDGET`] bytes.
//!
//! The first caller computes; callers asking for the same entry meanwhile wait for it (the compositor asks
//! from several tiles at once).

use super::maps::SetMaps;
use crate::paint::TextureSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// Bytes of generator results kept.
pub const FIELD_BUDGET: usize = 768 << 20;
/// Texture sets whose maps are kept.
const MAP_SETS: usize = 3;
/// Texels per side sampled from each baked map for its fingerprint.
const PROBES: u32 = 16;

/// A texture set's identity for the caches: id, resolution and a sample of every baked map.
pub fn fingerprint(set: &TextureSet) -> u64 {
    let mut h = DefaultHasher::new();
    set.id.hash(&mut h);
    set.resolution.hash(&mut h);
    for b in &set.baked {
        b.kind.id().hash(&mut h);
        let (w, ht) = b.image.size();
        (w, ht).hash(&mut h);
        for j in 0..PROBES {
            for i in 0..PROBES {
                let (x, y) = (i * w / PROBES, j * ht / PROBES);
                for c in b.image.get(x.min(w.saturating_sub(1)), y.min(ht.saturating_sub(1))) {
                    c.to_bits().hash(&mut h);
                }
            }
        }
    }
    h.finish()
}

/// A generator result: `w × h` values 0..1 as 16-bit steps, rows top first.
pub struct Field {
    pub w: usize,
    pub h: usize,
    px: Vec<u16>,
}

impl Field {
    pub fn from_values(w: usize, h: usize, values: &[f32]) -> Field {
        let px = values.iter().map(|v| (super::tone::unit(*v) * 65535.0).round() as u16).collect();
        Field { w, h, px }
    }

    pub fn at(&self, x: usize, y: usize) -> f32 {
        self.px.get(y * self.w + x).map_or(0.0, |v| *v as f32 / 65535.0)
    }

    fn bytes(&self) -> usize {
        self.px.len() * 2
    }
}

type Slot<T> = Arc<OnceLock<T>>;

struct Lru<T> {
    entries: Vec<(u64, Slot<T>)>,
}

impl<T> Lru<T> {
    /// The entry for `key`, moved to the back (most recent); a new empty one if absent.
    fn slot(&mut self, key: u64) -> Slot<T> {
        let slot = match self.entries.iter().position(|(k, _)| *k == key) {
            Some(i) => self.entries.remove(i).1,
            None => Arc::new(OnceLock::new()),
        };
        self.entries.push((key, slot.clone()));
        slot
    }

    /// Drops the oldest entries while `too_big` says so (the newest is always kept).
    fn trim(&mut self, too_big: impl Fn(&[(u64, Slot<T>)]) -> bool) {
        while self.entries.len() > 1 && too_big(&self.entries) {
            self.entries.remove(0);
        }
    }
}

static MAPS: Mutex<Lru<SetMaps>> = Mutex::new(Lru { entries: Vec::new() });
static FIELDS: Mutex<Lru<Field>> = Mutex::new(Lru { entries: Vec::new() });

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The set's maps, built on first use.
pub fn set_maps(set: &TextureSet, key: u64) -> Arc<OnceLock<SetMaps>> {
    let slot = {
        let mut maps = lock(&MAPS);
        let slot = maps.slot(key);
        maps.trim(|e| e.len() > MAP_SETS);
        slot
    };
    slot.get_or_init(|| SetMaps::of(set));
    slot
}

/// The key of one generator result.
pub fn field_key(set_key: u64, generator: &str, params: &[(String, String)]) -> u64 {
    let mut h = DefaultHasher::new();
    set_key.hash(&mut h);
    generator.hash(&mut h);
    params.hash(&mut h);
    h.finish()
}

/// The cached result for `key`, computed with `compute` on first use.
pub fn field(key: u64, compute: impl FnOnce() -> Field) -> Arc<OnceLock<Field>> {
    let slot = lock(&FIELDS).slot(key);
    slot.get_or_init(compute);
    let mut fields = lock(&FIELDS);
    fields.trim(|e| e.iter().filter_map(|(_, s)| s.get()).map(Field::bytes).sum::<usize>() > FIELD_BUDGET);
    slot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_computed_once_and_fingerprints_follow_the_maps() {
        let key = field_key(1, "test-once", &[("a".into(), "1".into())]);
        let first = field(key, || Field::from_values(2, 1, &[0.25, 2.0]));
        let again = field(key, || Field::from_values(2, 1, &[0.9, 0.9]));
        let f = again.get().unwrap();
        assert!((f.at(0, 0) - 0.25).abs() < 1e-4 && f.at(1, 0) == 1.0);
        assert!(Arc::ptr_eq(&first, &again));
        assert_ne!(key, field_key(1, "test-once", &[("a".into(), "2".into())]));
        let mut set = TextureSet::new("s", "S", (4, 4));
        let before = fingerprint(&set);
        let mut img = crate::paint::TiledImage::new(4, 4, crate::paint::PixelFormat::from_id("gray8").unwrap()).unwrap();
        set.baked.push(crate::paint::BakedMap { kind: crate::paint::BakedKind::Curvature, image: img.clone() });
        let with_map = fingerprint(&set);
        assert_ne!(before, with_map);
        img.fill([1.0; 4]);
        set.baked[0].image = img;
        assert_ne!(with_map, fingerprint(&set));
    }
}
