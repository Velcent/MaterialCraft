//! Spot looks: black-and-white spots ([`BnwSpots`]) and soft gaussian-like spots ([`GaussianSpots`]).

use super::cells;
use super::look::Look;
use crate::noise::{BnwSpots, GaussianSpots};
use crate::paint::effect::Params;

struct Bnw(BnwSpots);

impl Look for Bnw {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

struct Gaussian(GaussianSpots);

impl Look for Gaussian {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

fn bnw(style: u32, base: i32, p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(Bnw(BnwSpots { style, scale: cells(p, base), seed }))
}

/// Round spots of mixed sizes.
pub fn bnw_1(p: &Params, seed: u32) -> Box<dyn Look> {
    bnw(0, 8, p, seed)
}

/// Larger, crowded spots.
pub fn bnw_2(p: &Params, seed: u32) -> Box<dyn Look> {
    bnw(1, 6, p, seed)
}

/// Small spots with grainy edges.
pub fn bnw_3(p: &Params, seed: u32) -> Box<dyn Look> {
    bnw(2, 12, p, seed)
}

fn gaussian(base: i32, size: f32, p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(Gaussian(GaussianSpots {
        scale: cells(p, base),
        seed,
        size: p.range("size", size, 0.05, 1.0),
        size_variation: p.range("size_variation", 0.5, 0.0, 1.0),
    }))
}

/// Soft spots, few and large (`size`, `size_variation`).
pub fn gaussian_1(p: &Params, seed: u32) -> Box<dyn Look> {
    gaussian(8, 0.5, p, seed)
}

/// Soft spots, many and overlapping.
pub fn gaussian_2(p: &Params, seed: u32) -> Box<dyn Look> {
    gaussian(16, 0.75, p, seed)
}
