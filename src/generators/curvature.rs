//! `curvature`: a mask from the baked curvature mixed over several radii (fine edges to broad shapes), with
//! an optional grunge texture over it.
//!
//! Parameters: the scales `curv_sharp` … `curv_huge` and `curv_mode` (see [`super::parts::MultiScale`]),
//! `curv_contrast`, `curv_brightness`, `curv_invert`, a texture layer `tex1_…`, `seed`, and the finish
//! `invert`, `blur`, `balance`, `contrast`.

use super::maps::SetMaps;
use super::params::Params;
use super::parts::{Finish, MultiScale, TextureLayer};
use super::texel::{Recipe, Texel};
use super::tone::{brightness_contrast, invert_if};

pub struct Curvature;

pub struct Settings {
    scales: MultiScale,
    contrast: f32,
    brightness: f32,
    invert: bool,
    texture: Option<TextureLayer>,
    finish: Finish,
}

impl Recipe for Curvature {
    type Settings = Settings;

    fn settings(&self, p: &Params) -> Settings {
        Settings {
            scales: MultiScale::read(p, "curv"),
            contrast: p.range("curv_contrast", 0.0, -1.0, 1.0),
            brightness: p.range("curv_brightness", 0.0, -1.0, 1.0),
            invert: p.on("curv_invert", false),
            texture: TextureLayer::read(p, "tex1", p.seed("seed")),
            finish: Finish::read(p),
        }
    }

    fn value(&self, s: &Settings, t: &Texel, maps: &SetMaps) -> f32 {
        let c = s.scales.value(t, maps, s.finish.blur);
        let mut v = invert_if(brightness_contrast(c, s.brightness, s.contrast), s.invert);
        if let Some(tex) = &s.texture {
            v = tex.over(v, t, maps);
        }
        s.finish.apply(v)
    }
}
