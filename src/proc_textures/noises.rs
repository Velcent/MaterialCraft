//! Noise looks: fractal clouds, Perlin, fractal sum, moisture blotches, anisotropic streaks, fibers and white
//! noise, from the tileable lattices in [`crate::noise`].

use super::cells;
use super::look::Look;
use crate::noise::{Fbm, GradGrid, rnd, smoothstep};
use crate::paint::effect::Params;

struct FbmLook(Fbm);

impl Look for FbmLook {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.sample(u, v)
    }
}

fn fbm(p: &Params, seed: u32, base_cells: i32, octaves: u32, gain: f32) -> Box<dyn Look> {
    let c = cells(p, base_cells);
    Box::new(FbmLook(Fbm::new(c, c, octaves, gain, seed, 0)))
}

/// Large soft clouds.
pub fn clouds_1(p: &Params, seed: u32) -> Box<dyn Look> {
    fbm(p, seed, 3, 4, 0.45)
}

/// Clouds with more detail.
pub fn clouds_2(p: &Params, seed: u32) -> Box<dyn Look> {
    fbm(p, seed, 4, 6, 0.55)
}

/// Rough, fine-grained clouds.
pub fn clouds_3(p: &Params, seed: u32) -> Box<dyn Look> {
    fbm(p, seed, 6, 7, 0.62)
}

/// Many octaves from a very coarse start: large shapes with fine detail.
pub fn fractal_sum(p: &Params, seed: u32) -> Box<dyn Look> {
    fbm(p, seed, 2, 8, 0.5)
}

struct Perlin(GradGrid);

impl Look for Perlin {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.sample(u, v)
    }
}

/// One octave of gradient noise.
pub fn perlin(p: &Params, seed: u32) -> Box<dyn Look> {
    let c = cells(p, 8);
    Box::new(Perlin(GradGrid::new(c, c, seed)))
}

/// Soft wet blotches: domain-warped clouds pushed towards their edges.
struct Moisture {
    base: Fbm,
    warp_u: Fbm,
    warp_v: Fbm,
}

impl Look for Moisture {
    fn value(&self, u: f32, v: f32) -> f32 {
        let (du, dv) = (self.warp_u.sample(u, v) - 0.5, self.warp_v.sample(u, v) - 0.5);
        let n = self.base.sample((u + du * 0.35).rem_euclid(1.0), (v + dv * 0.35).rem_euclid(1.0));
        smoothstep(0.3, 0.7, n)
    }
}

pub fn moisture(p: &Params, seed: u32) -> Box<dyn Look> {
    let c = cells(p, 5);
    let w = cells(p, 3);
    Box::new(Moisture {
        base: Fbm::new(c, c, 6, 0.55, seed, 0),
        warp_u: Fbm::new(w, w, 3, 0.5, seed.wrapping_add(11), 0),
        warp_v: Fbm::new(w, w, 3, 0.5, seed.wrapping_add(12), 0),
    })
}

/// Value noise on a stretched lattice: `across` cells across the streaks, `along` cells along them;
/// `smoothness` 0 keeps hard steps across, 1 interpolates smoothly. `vertical` turns the streaks by 90°.
struct Streaks {
    across: i32,
    along: i32,
    smoothness: f32,
    vertical: bool,
    seed: u32,
    /// (weight, cells factor) of a finer second layer.
    detail: Option<(f32, i32)>,
}

impl Streaks {
    fn layer(&self, a: f32, b: f32, across: i32, along: i32, seed: u32) -> f32 {
        // b runs across the streaks (many cells), a along them (few cells)
        let (x, y) = (a * along as f32, b * across as f32);
        let (i, j) = (x.floor() as i32, y.floor() as i32);
        let (fx, fy) = (x - i as f32, y - j as f32);
        let sx = smoothstep(0.0, 1.0, fx);
        let sy = fy + (smoothstep(0.0, 1.0, fy) - fy) * self.smoothness;
        let sy = if self.smoothness <= 0.0 { if fy < 0.5 { 0.0 } else { 1.0 } } else { sy };
        let at = |di: i32, dj: i32| rnd((i + di).rem_euclid(along), (j + dj).rem_euclid(across), seed);
        let top = at(0, 0) + (at(1, 0) - at(0, 0)) * sx;
        let bottom = at(0, 1) + (at(1, 1) - at(0, 1)) * sx;
        top + (bottom - top) * sy
    }
}

impl Look for Streaks {
    fn value(&self, u: f32, v: f32) -> f32 {
        let (a, b) = if self.vertical { (v, u) } else { (u, v) };
        let base = self.layer(a, b, self.across, self.along, self.seed);
        match self.detail {
            Some((w, k)) => base * (1.0 - w) + w * self.layer(a, b, (self.across * k).min(4096), (self.along * k).min(4096), self.seed.wrapping_add(5)),
            None => base,
        }
    }
}

/// Long thin streaks (anisotropic value noise). `x_amount` cells along, `y_amount` across, `smoothness`,
/// `rotate` (1: vertical).
pub fn anisotropic(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(Streaks {
        across: p.int("y_amount", 128).clamp(1, 4096),
        along: p.int("x_amount", 4).clamp(1, 4096),
        smoothness: p.range("smoothness", 1.0, 0.0, 1.0),
        vertical: p.on("rotate", false),
        seed,
        detail: None,
    })
}

/// Dense fibres: fine streaks with a finer layer on top.
pub fn fibers(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(Streaks { across: cells(p, 256), along: cells(p, 6), smoothness: 1.0, vertical: p.on("rotate", false), seed, detail: Some((0.4, 2)) })
}

struct White {
    cells: i32,
    seed: u32,
}

impl Look for White {
    fn value(&self, u: f32, v: f32) -> f32 {
        let n = self.cells;
        rnd(((u * n as f32) as i32).rem_euclid(n), ((v * n as f32) as i32).rem_euclid(n), self.seed)
    }
}

/// A random value per cell (`scale` × 256 cells per side).
pub fn white_noise(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(White { cells: cells(p, 256), seed })
}
