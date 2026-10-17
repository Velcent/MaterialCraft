//! `metal_edge_wear`: wear on exposed convex edges, less of it in occluded places, broken up by patchy noise.
//!
//! Parameters: `level` (0..1, how far the wear reaches), `contrast` (0..1), `grunge_amount` (0..1),
//! `grunge_scale`, `smoothness` (edge softening, about texels at 1K), `ao_masking` (0..1),
//! `curvature_weight` (0..1), `triplanar` (0/1), `triplanar_contrast`, `seed`, `invert` (0/1).

use super::breakup::{Breakup, Placement};
use super::maps::SetMaps;
use super::params::Params;
use super::texel::{Recipe, Texel};
use super::tone::{invert_if, threshold, unit};

/// One smoothness step as a fraction of the texture width.
const SMOOTHNESS_UNIT: f32 = 1.0 / 1024.0;

pub struct EdgeWear;

pub struct Settings {
    level: f32,
    contrast: f32,
    grunge_amount: f32,
    blur: f32,
    ao_masking: f32,
    weight: f32,
    invert: bool,
    breakup: Breakup,
}

impl Recipe for EdgeWear {
    type Settings = Settings;

    fn settings(&self, p: &Params) -> Settings {
        let placement = Placement::of(p.on("triplanar", false), p.f("triplanar_contrast", 0.5));
        Settings {
            level: p.range("level", 0.75, 0.0, 1.0),
            contrast: p.range("contrast", 0.5, 0.0, 1.0),
            grunge_amount: p.range("grunge_amount", 0.3, 0.0, 1.0),
            blur: p.range("smoothness", 1.0, 0.0, 64.0) * SMOOTHNESS_UNIT,
            ao_masking: p.range("ao_masking", 0.5, 0.0, 1.0),
            weight: p.range("curvature_weight", 1.0, 0.0, 1.0),
            invert: p.on("invert", false),
            breakup: Breakup::new(p.range("grunge_scale", 4.0, 0.1, 64.0), p.seed("seed"), placement),
        }
    }

    fn value(&self, s: &Settings, t: &Texel, maps: &SetMaps) -> f32 {
        let edge = unit((maps.curvature(t.u, t.v, s.blur) - 0.5) * 2.0 * s.weight);
        let open = maps.ao(t.u, t.v, 0.002);
        let mut wear = edge * (1.0 - s.ao_masking * (1.0 - open));
        wear -= (1.0 - s.breakup.value(t, maps)) * s.grunge_amount;
        invert_if(threshold(wear * 1.5, s.level, s.contrast), s.invert)
    }
}
