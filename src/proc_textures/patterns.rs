//! Pattern looks: stripes, checker, weaves, triangle grids and starbursts.

use super::cells;
use super::look::Look;
use crate::noise::{Starburst, TriangleGrid, TriangleLattice, Weave, WeaveGenerator, smoothstep};
use crate::paint::effect::Params;

/// Parallel stripes: `count` per tile, `width` (0..1 of a period), `softness`, `shift` (the stripes lean by
/// that many periods over the tile, so they stay tileable), `vertical`.
struct Stripes {
    count: f32,
    width: f32,
    softness: f32,
    shift: f32,
    vertical: bool,
}

impl Look for Stripes {
    fn value(&self, u: f32, v: f32) -> f32 {
        let (a, b) = if self.vertical { (v, u) } else { (u, v) };
        let t = (b * self.count + a * self.shift).rem_euclid(1.0);
        // distance from the stripe's centre line, in periods
        let d = (t - 0.5).abs();
        let half = self.width * 0.5;
        let soft = (self.softness * half).max(1e-4);
        1.0 - smoothstep(half - soft, half, d)
    }
}

pub fn stripes(p: &Params, _seed: u32) -> Box<dyn Look> {
    Box::new(Stripes {
        count: p.int("count", 10).clamp(1, 1024) as f32,
        width: p.range("width", 0.5, 0.0, 1.0),
        softness: p.range("softness", 0.0, 0.0, 1.0),
        shift: p.int("shift", 0).clamp(-256, 256) as f32,
        vertical: p.on("vertical", false),
    })
}

struct Checker {
    tiles: i32,
}

impl Look for Checker {
    fn value(&self, u: f32, v: f32) -> f32 {
        let n = self.tiles as f32;
        (((u * n).floor() as i32 + (v * n).floor() as i32) & 1) as f32
    }
}

/// A checkerboard of `tiles` × `tiles` (even, so it tiles).
pub fn checker(p: &Params, _seed: u32) -> Box<dyn Look> {
    let tiles = p.int("tiles", 8).clamp(1, 512);
    Box::new(Checker { tiles: tiles + (tiles & 1) })
}

struct WeaveLook(Weave);

impl Look for WeaveLook {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// Threads over and under each other (`width`).
pub fn weave(p: &Params, _seed: u32) -> Box<dyn Look> {
    Box::new(WeaveLook(Weave { count: (cells(p, 3) * 2) as f32, width: p.range("width", 0.8, 0.1, 1.0) }))
}

struct WeaveGaps(WeaveGenerator);

impl Look for WeaveGaps {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// Threads in slots with gaps (`shape`, `gap_x`, `gap_y`).
pub fn weave_gaps(p: &Params, _seed: u32) -> Box<dyn Look> {
    let tile = cells(p, 1).min(20);
    let gap = (p.range("gap_x", 0.1, 0.0, 0.9), p.range("gap_y", 0.1, 0.0, 0.9));
    Box::new(WeaveGaps(WeaveGenerator::new((tile, tile), 2, p.range("shape", 0.5, 0.0, 1.0), gap)))
}

struct Grid(TriangleGrid);

impl Look for Grid {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// Squares split into two triangles with a random value each (`border`, `variation`).
pub fn triangle_grid(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(Grid(TriangleGrid { count: cells(p, 8), border: p.range("border", 0.06, 0.0, 0.5), variation: p.range("variation", 1.0, 0.0, 1.0), seed }))
}

struct Lattice(TriangleLattice);

impl Look for Lattice {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// A jittered triangle mesh with interpolated heights (`output` 0 heights, 1 flat, 2 edge distance).
pub fn triangles(p: &Params, seed: u32) -> Box<dyn Look> {
    let n = cells(p, 12).min(64);
    Box::new(Lattice(TriangleLattice {
        nx: n,
        ny: n,
        amp: p.range("jitter", 1.0, 0.0, 1.0) * 0.32,
        quincunx: (0.0, 0.0),
        height_random: p.range("height_random", 1.0, 0.0, 1.0),
        triangulation: 2,
        output: p.int("output", 0).clamp(0, 2),
        line_thickness: 0.0,
        line_color: 1.0,
        line_background: 0.0,
        lines: false,
        seed,
    }))
}

struct Star(Starburst);

impl Look for Star {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// Star-like rays in a grid (`rays`, `sharpness`, `size`, `rotation_random`).
pub fn starburst(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(Star(Starburst {
        count: cells(p, 3),
        rays: p.range("rays", 8.0, 2.0, 64.0).round(),
        sharpness: p.range("sharpness", 2.0, 0.1, 16.0),
        size: p.range("size", 1.0, 0.05, 2.0),
        rotation: 0.0,
        rotation_random: p.range("rotation_random", 0.0, 0.0, 1.0),
        seed,
    }))
}
