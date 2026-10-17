//! `ambient_occlusion`: a mask from the baked ambient occlusion (open areas white; invert for cavities), with
//! an optional grunge texture over it.
//!
//! Parameters: `ao_invert`, `ao_blur`, `ao_balance`, `ao_contrast`, a texture layer `tex1_…` (see
//! [`super::parts`]), `seed`, and the finish `invert`, `blur`, `balance`, `contrast`.

use super::maps::SetMaps;
use super::params::Params;
use super::parts::{Finish, TextureLayer, blur_fraction};
use super::texel::{Recipe, Texel};
use super::tone::{invert_if, scan};

pub struct AmbientOcclusion;

pub struct Settings {
    invert: bool,
    blur: f32,
    balance: f32,
    contrast: f32,
    texture: Option<TextureLayer>,
    finish: Finish,
}

impl Recipe for AmbientOcclusion {
    type Settings = Settings;

    fn settings(&self, p: &Params) -> Settings {
        let finish = Finish::read(p);
        Settings {
            invert: p.on("ao_invert", false),
            blur: blur_fraction(p.f("ao_blur", 0.0)) + finish.blur,
            balance: p.range("ao_balance", 0.5, 0.0, 1.0),
            contrast: p.range("ao_contrast", 0.0, 0.0, 1.0),
            texture: TextureLayer::read(p, "tex1", p.seed("seed")),
            finish,
        }
    }

    fn value(&self, s: &Settings, t: &Texel, maps: &SetMaps) -> f32 {
        let mut v = scan(invert_if(maps.ao(t.u, t.v, s.blur), s.invert), s.balance, s.contrast);
        if let Some(tex) = &s.texture {
            v = tex.over(v, t, maps);
        }
        s.finish.apply(v)
    }
}
