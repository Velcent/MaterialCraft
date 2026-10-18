// Copied from PhotoCraft, crates/algo/src/artistic_fx.rs (the Texturizer's `texture` fields, `weave`, `vnoise`,
// `fbm`, `smoothstep`) and crates/algo/src/noise.rs (`hash01`), revision 5f65579. Changed: the value noise wraps
// on a lattice that fits a whole number of cells into the tile and the sandstone grain wraps with the tile, so every
// field tiles; the fields are an enum instead of names. See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! [`Field`]: the height fields of PhotoCraft's Texturizer filter (brick, burlap, canvas, sandstone, blocks, frosted,
//! tiny lens), 0..1, made seamless over a [`TILE`]-unit square.

use std::f32::consts::PI;

/// Side of one tile, in field units: a common multiple of every field's own period (brick 24, blocks 18, tiny lens 12,
/// burlap 14, canvas 7), so the geometric fields tile; the noise is wrapped to it.
pub const TILE: f32 = 504.0;

/// One Texturizer field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Brick,
    Burlap,
    Canvas,
    Sandstone,
    Blocks,
    Frosted,
    TinyLens,
}

impl Field {
    /// The field's name in PhotoCraft's Texturizer, as provenance records it.
    pub fn id(self) -> &'static str {
        match self {
            Field::Brick => "brick",
            Field::Burlap => "burlap",
            Field::Canvas => "canvas",
            Field::Sandstone => "sandstone",
            Field::Blocks => "blocks",
            Field::Frosted => "frosted",
            Field::TinyLens => "tinyLens",
        }
    }

    /// The height at `(x, y)` field units (0..1); repeats every [`TILE`] units in both directions.
    pub fn height(self, x: f32, y: f32) -> f32 {
        match self {
            Field::Brick => {
                let (bw, bh) = (24.0, 12.0);
                let row = (y / bh).floor();
                let xo = x + if row as i32 % 2 == 0 { 0.0 } else { bw / 2.0 };
                let (u, v) = (xo.rem_euclid(bw), y.rem_euclid(bh));
                let d = u.min(bw - u).min(v.min(bh - v));
                smoothstep(0.6, 2.2, d) * (0.85 + 0.15 * noise(x, y, 3.0, 3.0, 91))
            }
            Field::Burlap => weave(x, y, 7.0, 0.35),
            Field::Canvas => weave(x, y, 3.5, 0.18),
            Field::Sandstone => {
                let t = TILE as i32;
                fbm(x, y, 7.0, 93) * 0.7 + hash01((x.floor() as i32).rem_euclid(t), (y.floor() as i32).rem_euclid(t), 0, 94) * 0.3
            }
            Field::Blocks => {
                let b = 18.0;
                let (u, v) = (x.rem_euclid(b) / b, y.rem_euclid(b) / b);
                (u.min(1.0 - u).min(v.min(1.0 - v)) * 4.0).min(1.0)
            }
            Field::Frosted => fbm(x, y, 2.5, 95),
            Field::TinyLens => {
                let p = 12.0;
                let (u, v) = ((x.rem_euclid(p) / p - 0.5) * 2.0, (y.rem_euclid(p) / p - 0.5) * 2.0);
                (1.0 - (u * u + v * v)).max(0.0).sqrt()
            }
        }
    }
}

/// Hermite step from `e0` to `e1`; `e0 > e1` gives the falling step.
fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if (e1 - e0).abs() < 1e-9 {
        return if x >= e1 { 1.0 } else { 0.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Deterministic hash → `[0, 1)` of integer coordinates.
fn hash01(x: i32, y: i32, c: u32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ c.wrapping_mul(0xcb1a_b31f) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^= h >> 16;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Cells of about `size` units that fit a whole number of times into the tile.
fn cells(size: f32) -> i32 {
    ((TILE / size.max(1e-3)).round() as i32).max(1)
}

/// Smooth value noise (0–1) with cells of about `sx` × `sy` units, wrapping over the tile.
fn noise(x: f32, y: f32, sx: f32, sy: f32, seed: u32) -> f32 {
    let (cx, cy) = (cells(sx), cells(sy));
    vnoise(x * cx as f32 / TILE, y * cy as f32 / TILE, seed, (cx, cy))
}

/// Smooth value noise (0–1) on a unit lattice that repeats every `period` cells.
fn vnoise(x: f32, y: f32, seed: u32, period: (i32, i32)) -> f32 {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (tx, ty) = (x - ix as f32, y - iy as f32);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let h = |a: i32, b: i32| hash01(a.rem_euclid(period.0), b.rem_euclid(period.1), 0, seed);
    let top = h(ix, iy) + (h(ix + 1, iy) - h(ix, iy)) * sx;
    let bot = h(ix, iy + 1) + (h(ix + 1, iy + 1) - h(ix, iy + 1)) * sx;
    top + (bot - top) * sy
}

/// Three-octave value noise (0–1) with base cells of about `size` units.
fn fbm(x: f32, y: f32, size: f32, seed: u32) -> f32 {
    noise(x, y, size, size, seed) * 0.57 + noise(x, y, size / 2.03, size / 2.03, seed ^ 0x51) * 0.29 + noise(x, y, size / 4.1, size / 4.1, seed ^ 0xa3) * 0.14
}

/// Basket weave of threads `p` units wide with `noise` irregularity.
fn weave(x: f32, y: f32, p: f32, irregular: f32) -> f32 {
    let (cx, cy) = ((x / p).floor() as i32, (y / p).floor() as i32);
    let over_h = (cx + cy).rem_euclid(2) == 0;
    let (u, v) = (x.rem_euclid(p) / p, y.rem_euclid(p) / p);
    // A thread is a rounded ridge across its cell.
    let ridge = if over_h { (v * PI).sin() } else { (u * PI).sin() };
    let n = noise(x, y, p * 0.7, p * 2.0, 97);
    ridge * (1.0 - irregular) + n * irregular
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Field; 7] = [Field::Brick, Field::Burlap, Field::Canvas, Field::Sandstone, Field::Blocks, Field::Frosted, Field::TinyLens];

    #[test]
    fn every_field_tiles_and_stays_in_range() {
        for f in ALL {
            for i in 0..200 {
                let (x, y) = ((i * 37 % 504) as f32 + 0.25, (i * 91 % 504) as f32 + 0.5);
                let h = f.height(x, y);
                assert!((0.0..=1.0).contains(&h), "{f:?} {h}");
                assert!((h - f.height(x + TILE, y)).abs() < 1e-3, "{f:?} at {x} {y}");
                assert!((h - f.height(x, y + TILE)).abs() < 1e-3, "{f:?} at {x} {y}");
            }
        }
    }
}
