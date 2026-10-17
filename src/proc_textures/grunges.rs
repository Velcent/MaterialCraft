//! Grunge looks: the fifteen maps of [`crate::grunge`] (thresholded with their own balance and contrast), soft
//! dirt and rust fields made from the same dirt recipes, and the four looks of [`GrungeVariant`].

use super::look::Look;
use crate::grunge::{GrungeP, GrungeVariant, T0, Variant, dirt as dirt_field, grunge_map};
use crate::noise::smoothstep;
use crate::paint::effect::Params;

/// One map of the grunge family. `balance` (0.5), `contrast` (0.5), `brush` (0), `cracks` (1) and `coverage`
/// (the map's own threshold) shape it.
pub struct GrungeMap {
    look: GrungeP,
    threshold: f32,
}

impl GrungeMap {
    /// Map `map` (1..=15) with the given settings.
    pub fn new(map: u32, seed: u32, balance: f32, contrast: f32, brush: f32, cracks: f32, coverage: Option<f32>) -> GrungeMap {
        let map = map.clamp(1, 15);
        let threshold = coverage.unwrap_or_else(|| T0.get(map as usize - 1).copied().unwrap_or(0.5));
        GrungeMap { look: GrungeP { map, seed, balance, contrast, brush, cracks, disorder: 0.0 }, threshold }
    }

    /// Map `map` from the parameters.
    pub fn read(map: u32, p: &Params, seed: u32) -> GrungeMap {
        let coverage = p.f("coverage", f32::NAN);
        GrungeMap::new(
            map,
            seed,
            p.range("balance", 0.5, 0.0, 1.0),
            p.range("contrast", 0.5, 0.0, 1.0),
            p.range("brush", 0.0, 0.0, 1.0),
            p.range("cracks", 1.0, 0.0, 1.0),
            coverage.is_finite().then_some(coverage.clamp(0.0, 1.5)),
        )
    }
}

impl Look for GrungeMap {
    fn value(&self, u: f32, v: f32) -> f32 {
        grunge_map(&self.look, u, v, self.threshold)
    }

    fn own_balance(&self) -> bool {
        true
    }
}

macro_rules! maps {
    ($($name:ident = $map:expr),* $(,)?) => {
        $(
            pub fn $name(p: &Params, seed: u32) -> Box<dyn Look> {
                Box::new(GrungeMap::read($map, p, seed))
            }
        )*
    };
}

maps!(
    map_01 = 1,
    map_02 = 2,
    map_03 = 3,
    map_04 = 4,
    map_05 = 5,
    map_06 = 6,
    map_07 = 7,
    map_08 = 8,
    map_09 = 9,
    map_10 = 10,
    map_11 = 11,
    map_12 = 12,
    map_13 = 13,
    map_14 = 14,
    map_15 = 15,
    leaks = 2,
    plaster_cracks = 11,
);

/// A grunge map's dirt field as a soft grey value: white where clean, darker with more dirt.
struct SoftDirt {
    look: GrungeP,
    /// The dirt amounts mapped to white and to black.
    clean: f32,
    dirty: f32,
}

impl Look for SoftDirt {
    fn value(&self, u: f32, v: f32) -> f32 {
        1.0 - smoothstep(self.clean, self.dirty, dirt_field(&self.look, u, v))
    }
}

fn soft(map: u32, seed: u32, clean: f32, dirty: f32) -> Box<dyn Look> {
    Box::new(SoftDirt { look: GrungeP { map, seed, balance: 0.5, contrast: 0.5, brush: 0.0, cracks: 1.0, disorder: 0.0 }, clean, dirty })
}

/// Soft blotchy dirt with specks.
pub fn dirt(_p: &Params, seed: u32) -> Box<dyn Look> {
    soft(1, seed, 0.15, 1.1)
}

/// Fine rust grain over larger patches.
pub fn rust_fine(_p: &Params, seed: u32) -> Box<dyn Look> {
    soft(9, seed, 0.3, 1.0)
}

struct Variant4(GrungeVariant);

impl Look for Variant4 {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

fn variant(variant: Variant, p: &Params, seed: u32) -> Box<dyn Look> {
    let mut g = GrungeVariant::new(variant, seed);
    g.dirt_specks = p.range("specks", 0.5, 0.0, 1.0);
    g.scratches = p.range("scratch_lines", 0.3, 0.0, 1.0);
    g.quantity = p.int("quantity", 6).clamp(1, 32);
    g.scratch_width = p.range("scratch_width", 0.4, 0.05, 0.95);
    g.scratch_length = p.range("scratch_length", 0.45, 0.0, 0.9);
    g.details = p.int("details", 22).clamp(1, 128);
    g.coverage = p.range("coverage", 0.35, 0.0, 0.9);
    Box::new(Variant4(g))
}

/// Mottled concrete with specks and faint lines (`specks`, `scratch_lines`).
pub fn concrete(p: &Params, seed: u32) -> Box<dyn Look> {
    variant(Variant::Concrete, p, seed)
}

/// Long thin scratches (`quantity`, `scratch_width`, `scratch_length`).
pub fn scratches(p: &Params, seed: u32) -> Box<dyn Look> {
    variant(Variant::Scratches, p, seed)
}

/// Spots in patches (`details`, `coverage`).
pub fn spots(p: &Params, seed: u32) -> Box<dyn Look> {
    variant(Variant::Spots, p, seed)
}

/// Large soft smudges.
pub fn smudges(p: &Params, seed: u32) -> Box<dyn Look> {
    variant(Variant::Smudges, p, seed)
}
