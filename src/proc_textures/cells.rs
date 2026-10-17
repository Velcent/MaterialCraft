//! Cell looks: four cell styles ([`Cells`]), crystal facets ([`Crystal`]) and fractal Voronoi
//! ([`VoronoiFractal`]).

use super::cells as count;
use super::look::Look;
use crate::noise::{Cells, Crystal, VoronoiFractal, VoronoiSettings};
use crate::paint::effect::Params;

struct CellLook(Cells);

impl Look for CellLook {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

fn cell(style: u32, p: &Params, seed: u32) -> Box<dyn Look> {
    let mut c = Cells::new(style, count(p, 8), seed);
    c.jitter = p.range("jitter", 1.0, 0.0, 1.0);
    c.edge_width = p.range("edge_width", 0.25, 0.01, 1.0);
    c.hardness = p.range("hardness", 2.0, 0.1, 16.0);
    c.luminance_random = p.range("luminance_random", 0.0, 0.0, 1.0);
    Box::new(CellLook(c))
}

/// Soft cones, bright at the centres.
pub fn cells_1(p: &Params, seed: u32) -> Box<dyn Look> {
    cell(0, p, seed)
}

/// Dark borders between the cells (`edge_width`).
pub fn cells_2(p: &Params, seed: u32) -> Box<dyn Look> {
    cell(2, p, seed)
}

/// Cells with a random value and a sharp falloff (`hardness`).
pub fn cells_3(p: &Params, seed: u32) -> Box<dyn Look> {
    cell(3, p, seed)
}

/// Flat random value per cell.
pub fn cells_4(p: &Params, seed: u32) -> Box<dyn Look> {
    cell(1, p, seed)
}

struct CrystalLook(Crystal);

impl Look for CrystalLook {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// Faceted cells shaded like cones.
pub fn crystal_1(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(CrystalLook(Crystal { style: 0, scale: count(p, 6), seed }))
}

/// Faceted cells lit from one side.
pub fn crystal_2(p: &Params, seed: u32) -> Box<dyn Look> {
    Box::new(CrystalLook(Crystal { style: 1, scale: count(p, 6), seed }))
}

struct VoronoiLook(VoronoiFractal);

impl Look for VoronoiLook {
    fn value(&self, u: f32, v: f32) -> f32 {
        self.0.value(u, v)
    }
}

/// Several Voronoi levels summed (`style` 0..6, `levels`, `roughness`).
pub fn voronoi_fractal(p: &Params, seed: u32) -> Box<dyn Look> {
    let mut s = VoronoiSettings::new(count(p, 4), seed);
    s.style = p.int("style", 0).clamp(0, 6) as u32;
    s.max_level = p.int("levels", 3).clamp(0, 6) as u32;
    s.roughness = p.range("roughness", 0.5, 0.0, 1.5);
    Box::new(VoronoiLook(VoronoiFractal::new(&s, true)))
}
