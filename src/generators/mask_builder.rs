//! `mask_builder`: a weighted sum of mesh-map terms (occlusion, convex and concave curvature, a top-down
//! gradient, world-space normal directions), noise on top, then a level threshold.
//!
//! Parameters: `level`, `contrast`, `invert`; noise `grunge` (amount), `grunge_scale`, `triplanar`,
//! `triplanar_contrast`, `seed`; occlusion `ao` (weight, may be negative), `ao_range`, `ao_contrast`,
//! `ao_noise`; curvature `curvature` (weight), `convex_range`, `concave_range`, `curvature_smoothness`,
//! `curvature_boost`, `curvature_noise`; gradient `gradient` (weight), `gradient_range`, `gradient_contrast`,
//! `gradient_invert`; directions `normal` (weight) and `normal_top`, `normal_bottom`, `normal_front`,
//! `normal_back`, `normal_right`, `normal_left` (−1..1 each).

use super::breakup::{Breakup, Placement};
use super::maps::SetMaps;
use super::params::Params;
use super::texel::{Recipe, Texel};
use super::tone::{invert_if, scan, threshold, unit};

pub struct MaskBuilder;

pub struct Settings {
    level: f32,
    contrast: f32,
    invert: bool,
    grunge: f32,
    noise: Breakup,
    ao: (f32, f32, f32, f32),
    curvature: (f32, f32, f32, f32, f32, f32),
    gradient: (f32, f32, f32, bool),
    normal: (f32, [f32; 6]),
}

impl Recipe for MaskBuilder {
    type Settings = Settings;

    fn settings(&self, p: &Params) -> Settings {
        let placement = Placement::of(p.on("triplanar", false), p.f("triplanar_contrast", 0.5));
        let w = |k: &str, d: f32| p.range(k, d, -1.0, 1.0);
        let u = |k: &str, d: f32| p.range(k, d, 0.0, 1.0);
        Settings {
            level: u("level", 0.5),
            contrast: u("contrast", 0.0),
            invert: p.on("invert", false),
            grunge: u("grunge", 0.25),
            noise: Breakup::new(p.range("grunge_scale", 2.0, 0.1, 64.0), p.seed("seed"), placement),
            ao: (w("ao", 1.0), u("ao_range", 0.5), u("ao_contrast", 0.0), u("ao_noise", 0.5)),
            curvature: (
                w("curvature", 1.0),
                p.range("convex_range", 0.5, -1.0, 1.0),
                p.range("concave_range", 0.0, -1.0, 1.0),
                p.range("curvature_smoothness", 0.0, 0.0, 16.0) / 1024.0,
                u("curvature_boost", 0.25),
                u("curvature_noise", 0.25),
            ),
            gradient: (w("gradient", 0.0), u("gradient_range", 0.5), u("gradient_contrast", 0.5), p.on("gradient_invert", false)),
            normal: (w("normal", 0.0), ["normal_top", "normal_bottom", "normal_front", "normal_back", "normal_right", "normal_left"].map(|k| w(k, 0.0))),
        }
    }

    fn value(&self, s: &Settings, t: &Texel, maps: &SetMaps) -> f32 {
        let noise = s.noise.value(t, maps);
        let mut f = 0.0;
        let (ao_w, ao_range, ao_contrast, ao_noise) = s.ao;
        if ao_w != 0.0 {
            let occ = scan(1.0 - maps.ao(t.u, t.v, 0.002), ao_range, ao_contrast);
            f += ao_w * occ * (1.0 - ao_noise * (1.0 - noise));
        }
        let (c_w, convex, concave, smooth, boost, c_noise) = s.curvature;
        if c_w != 0.0 {
            let c = maps.curvature(t.u, t.v, smooth);
            let edges = scan(unit((c - 0.5) * 2.0 + boost), 0.5 + convex * 0.5, 0.0);
            let cavities = scan(unit((0.5 - c) * 2.0), 0.5 + concave * 0.5, 0.0) * if concave != 0.0 { 1.0 } else { 0.0 };
            f += c_w * edges.max(cavities) * (1.0 - c_noise * (1.0 - noise));
        }
        let (g_w, g_range, g_contrast, g_invert) = s.gradient;
        if g_w != 0.0 {
            let up = maps.position(t.u, t.v)[1];
            f += g_w * invert_if(scan(up, g_range, g_contrast), g_invert);
        }
        let (n_w, dirs) = s.normal;
        if n_w != 0.0 {
            let n = maps.normal(t.u, t.v);
            let facing = [n[1], -n[1], n[2], -n[2], n[0], -n[0]];
            let d: f32 = facing.iter().zip(dirs).map(|(f, k)| f.max(0.0) * k).sum();
            f += n_w * d;
        }
        f += (noise - 0.5) * 2.0 * s.grunge;
        invert_if(threshold(f, s.level, s.contrast), s.invert)
    }
}
