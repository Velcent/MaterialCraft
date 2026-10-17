//! `dirt`: dirt collects where the surface is occluded and in cavities, stays off exposed edges, and is
//! broken up by patchy noise.
//!
//! Parameters: `level` (0..1, how far the dirt reaches), `contrast` (0..1, edge sharpness), `grunge_amount`
//! (0..1), `grunge_scale` (repeats), `edge_masking` (0..1, keeps convex edges clean), `triplanar` (0/1),
//! `triplanar_contrast` (0..1), `seed`, `invert` (0/1).

use super::breakup::{Breakup, Placement};
use super::maps::SetMaps;
use super::params::Params;
use super::texel::{Recipe, Texel};
use super::tone::{invert_if, threshold, unit};

/// Blur of the occlusion read (fraction of the texture width): dirt pools softly.
const AO_BLUR: f32 = 0.004;
const CURVATURE_BLUR: f32 = 0.002;

pub struct Dirt;

pub struct Settings {
    level: f32,
    contrast: f32,
    grunge_amount: f32,
    edge_masking: f32,
    invert: bool,
    breakup: Breakup,
}

impl Recipe for Dirt {
    type Settings = Settings;

    fn settings(&self, p: &Params) -> Settings {
        let placement = Placement::of(p.on("triplanar", false), p.f("triplanar_contrast", 0.5));
        Settings {
            level: p.range("level", 0.75, 0.0, 1.0),
            contrast: p.range("contrast", 0.5, 0.0, 1.0),
            grunge_amount: p.range("grunge_amount", 0.3, 0.0, 1.0),
            edge_masking: p.range("edge_masking", 0.5, 0.0, 1.0),
            invert: p.on("invert", false),
            breakup: Breakup::new(p.range("grunge_scale", 4.0, 0.1, 64.0), p.seed("seed"), placement),
        }
    }

    fn value(&self, s: &Settings, t: &Texel, maps: &SetMaps) -> f32 {
        let occlusion = 1.0 - maps.ao(t.u, t.v, AO_BLUR);
        let c = maps.curvature(t.u, t.v, CURVATURE_BLUR);
        let cavity = unit((0.5 - c) * 2.0);
        let edge = unit((c - 0.5) * 2.0);
        let mut d = occlusion * 0.85 + cavity * 0.6 - edge * s.edge_masking;
        d += (s.breakup.value(t, maps) - 0.5) * 2.0 * s.grunge_amount;
        invert_if(threshold(d, s.level, s.contrast), s.invert)
    }
}
