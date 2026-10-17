//! `mask_editor`: a mask layered from up to seven parts, bottom to top: ambient occlusion (`ao_…`), curvature
//! (`curv_…`), world-space normal direction (`normal_…`), position gradient (`pos_…`), thickness (`thick_…`)
//! and two grunge textures (`tex1_…`, `tex2_…`). Each part has an `opacity` (0 leaves it out) and a `blend`
//! (see [`super::tone::Layering`]); the result gets the finish `invert`, `blur`, `balance`, `contrast`.
//!
//! Part parameters besides `opacity` and `blend`:
//! - ao: `invert`, `blur`, `balance`, `contrast`;
//! - curv: the scales and `mode` of [`super::parts::MultiScale`], `contrast`, `brightness`, `invert`;
//! - normal / pos: per axis a weight `x`, `y`, `z` and `x_invert` …, plus `invert`, `balance`, `contrast`,
//!   `brightness` (normal: up, right, front are white; position: top, right, front are white);
//! - thick: `invert`, `blur`, `contrast`, `brightness`;
//! - tex1 / tex2: see [`super::parts::TextureLayer`].

use super::maps::SetMaps;
use super::params::Params;
use super::parts::{Finish, MultiScale, TextureLayer, blur_fraction};
use super::texel::{Recipe, Texel};
use super::tone::{Layering, brightness_contrast, invert_if, scan};

pub struct MaskEditor;

/// Opacity and blend of one part.
struct Part {
    opacity: f32,
    blend: Layering,
}

impl Part {
    fn read(p: &Params, prefix: &str, blend: i32) -> Option<Part> {
        let opacity = p.range(&format!("{prefix}_opacity"), 0.0, 0.0, 1.0);
        (opacity > 0.0).then(|| Part { opacity, blend: Layering::from_index(p.int(&format!("{prefix}_blend"), blend)) })
    }
}

/// Tone settings most parts share.
struct Tone {
    invert: bool,
    balance: f32,
    contrast: f32,
    brightness: f32,
}

impl Tone {
    fn read(p: &Params, prefix: &str) -> Tone {
        let key = |k: &str| format!("{prefix}_{k}");
        Tone {
            invert: p.on(&key("invert"), false),
            balance: p.range(&key("balance"), 0.5, 0.0, 1.0),
            contrast: p.range(&key("contrast"), 0.0, -1.0, 1.0),
            brightness: p.range(&key("brightness"), 0.0, -1.0, 1.0),
        }
    }

    fn apply(&self, v: f32) -> f32 {
        let v = scan(v, self.balance, self.contrast.max(0.0));
        invert_if(brightness_contrast(v, self.brightness, self.contrast.min(0.0)), self.invert)
    }
}

/// Per-axis weights and inversions of a direction or gradient part.
struct Axes {
    weights: [f32; 3],
    inverts: [bool; 3],
}

impl Axes {
    fn read(p: &Params, prefix: &str) -> Axes {
        let axis = |a: &str, default: f32| p.range(&format!("{prefix}_{a}"), default, 0.0, 1.0);
        let inv = |a: &str| p.on(&format!("{prefix}_{a}_invert"), false);
        Axes { weights: [axis("x", 0.0), axis("y", 1.0), axis("z", 0.0)], inverts: [inv("x"), inv("y"), inv("z")] }
    }

    /// The weighted mean of the per-axis values (each 0..1).
    fn mix(&self, values: [f32; 3]) -> f32 {
        let total: f32 = self.weights.iter().sum();
        if total <= 1e-6 {
            return 0.0;
        }
        let sum: f32 = (0..3).map(|i| self.weights[i] * invert_if(values[i], self.inverts[i])).sum();
        sum / total
    }
}

pub struct Settings {
    ao: Option<(Part, Tone, f32)>,
    curv: Option<(Part, MultiScale, Tone)>,
    normal: Option<(Part, Axes, Tone)>,
    pos: Option<(Part, Axes, Tone)>,
    thick: Option<(Part, Tone, f32)>,
    tex1: Option<TextureLayer>,
    tex2: Option<TextureLayer>,
    finish: Finish,
}

impl Recipe for MaskEditor {
    type Settings = Settings;

    fn settings(&self, p: &Params) -> Settings {
        let finish = Finish::read(p);
        let seed = p.seed("seed");
        let blur = |prefix: &str| blur_fraction(p.f(&format!("{prefix}_blur"), 0.0)) + finish.blur;
        Settings {
            ao: Part::read(p, "ao", 3).map(|part| (part, Tone::read(p, "ao"), blur("ao"))),
            curv: Part::read(p, "curv", 1).map(|part| (part, MultiScale::read(p, "curv"), Tone::read(p, "curv"))),
            normal: Part::read(p, "normal", 1).map(|part| (part, Axes::read(p, "normal"), Tone::read(p, "normal"))),
            pos: Part::read(p, "pos", 1).map(|part| (part, Axes::read(p, "pos"), Tone::read(p, "pos"))),
            thick: Part::read(p, "thick", 1).map(|part| (part, Tone::read(p, "thick"), blur("thick"))),
            tex1: TextureLayer::read(p, "tex1", seed),
            tex2: TextureLayer::read(p, "tex2", seed.wrapping_add(1)),
            finish,
        }
    }

    fn value(&self, s: &Settings, t: &Texel, maps: &SetMaps) -> f32 {
        let mut v = 0.0;
        if let Some((part, tone, blur)) = &s.ao {
            v = part.blend.mix(v, tone.apply(maps.ao(t.u, t.v, *blur)), part.opacity);
        }
        if let Some((part, scales, tone)) = &s.curv {
            v = part.blend.mix(v, tone.apply(scales.value(t, maps, s.finish.blur)), part.opacity);
        }
        if let Some((part, axes, tone)) = &s.normal {
            let n = maps.normal(t.u, t.v).map(|c| c * 0.5 + 0.5);
            v = part.blend.mix(v, tone.apply(axes.mix(n)), part.opacity);
        }
        if let Some((part, axes, tone)) = &s.pos {
            let p = maps.position(t.u, t.v);
            v = part.blend.mix(v, tone.apply(axes.mix(p)), part.opacity);
        }
        if let Some((part, tone, blur)) = &s.thick {
            v = part.blend.mix(v, tone.apply(maps.thickness(t.u, t.v, *blur)), part.opacity);
        }
        for tex in [&s.tex1, &s.tex2].into_iter().flatten() {
            v = tex.over(v, t, maps);
        }
        s.finish.apply(v)
    }
}
