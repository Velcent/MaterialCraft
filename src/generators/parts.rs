//! Building blocks the layered generators (ambient occlusion, curvature, mask editor) share, each read from
//! parameters with a common prefix:
//!
//! - [`Finish`] (no prefix): `invert`, `blur`, `balance`, `contrast` applied to the result;
//! - [`TextureLayer`] (`<p>_…`): `opacity`, `blend`, `look`, `scale`, `contrast`, `brightness`, `invert`,
//!   `triplanar`, `triplanar_contrast`;
//! - [`MultiScale`] (`<p>_…`): curvature at seven radii (`sharp`, `fine`, `soft`, `medium`, `large`, `big`,
//!   `huge` weights) and a `mode` (1 both, 2 convex, 3 concave, 4 both as brightness).
//!
//! Blur values are in texels of a 256-texel texture (so they look the same at every resolution).

use super::breakup::{Placement, Texture};
use super::maps::SetMaps;
use super::params::Params;
use super::texel::Texel;
use super::tone::{Layering, brightness_contrast, invert_if, scan, unit};

/// A blur value as a fraction of the texture width.
pub fn blur_fraction(blur: f32) -> f32 {
    blur.clamp(0.0, 256.0) / 256.0
}

/// The final adjustments of a layered generator.
pub struct Finish {
    pub invert: bool,
    /// Fraction of the texture width.
    pub blur: f32,
    balance: f32,
    contrast: f32,
}

impl Finish {
    pub fn read(p: &Params) -> Finish {
        Finish {
            invert: p.on("invert", false),
            blur: blur_fraction(p.f("blur", 0.0)),
            balance: p.range("balance", 0.5, 0.0, 1.0),
            contrast: p.range("contrast", 0.0, 0.0, 1.0),
        }
    }

    pub fn apply(&self, v: f32) -> f32 {
        scan(invert_if(unit(v), self.invert), self.balance, self.contrast)
    }
}

/// A grunge texture laid over the result.
pub struct TextureLayer {
    pub opacity: f32,
    blend: Layering,
    texture: Texture,
    contrast: f32,
    brightness: f32,
    invert: bool,
}

impl TextureLayer {
    /// `None` when the layer's opacity is 0.
    pub fn read(p: &Params, prefix: &str, seed: u32) -> Option<TextureLayer> {
        let key = |k: &str| format!("{prefix}_{k}");
        let opacity = p.range(&key("opacity"), 0.0, 0.0, 1.0);
        if opacity <= 0.0 {
            return None;
        }
        let placement = Placement::of(p.on(&key("triplanar"), false), p.f(&key("triplanar_contrast"), 0.5));
        let look = p.int(&key("look"), 1).clamp(1, 15) as u32;
        Some(TextureLayer {
            opacity,
            blend: Layering::from_index(p.int(&key("blend"), 3)),
            texture: Texture::new(look, p.range(&key("scale"), 1.0, 0.05, 64.0), seed, placement),
            contrast: p.range(&key("contrast"), 0.0, -1.0, 1.0),
            brightness: p.range(&key("brightness"), 0.0, -1.0, 1.0),
            invert: p.on(&key("invert"), false),
        })
    }

    /// `below` with the texture laid over it.
    pub fn over(&self, below: f32, t: &Texel, maps: &SetMaps) -> f32 {
        let v = brightness_contrast(self.texture.value(t, maps), self.brightness, self.contrast);
        self.blend.mix(below, invert_if(v, self.invert), self.opacity)
    }
}

/// Radii of the seven curvature scales, as fractions of the texture width.
const RADII: [f32; 7] = [0.0, 1.0 / 1024.0, 2.0 / 1024.0, 4.0 / 1024.0, 8.0 / 1024.0, 16.0 / 1024.0, 32.0 / 1024.0];
const SCALES: [&str; 7] = ["sharp", "fine", "soft", "medium", "large", "big", "huge"];
const DEFAULT_WEIGHTS: [f32; 7] = [1.0, 0.9, 0.75, 0.5, 0.25, 0.15, 0.1];

/// Curvature mixed from several radii.
pub struct MultiScale {
    weights: [f32; 7],
    mode: i32,
}

impl MultiScale {
    pub fn read(p: &Params, prefix: &str) -> MultiScale {
        let mut weights = DEFAULT_WEIGHTS;
        for (w, name) in weights.iter_mut().zip(SCALES) {
            *w = p.range(&format!("{prefix}_{name}"), *w, 0.0, 1.0);
        }
        MultiScale { weights, mode: p.int(&format!("{prefix}_mode"), 1) }
    }

    /// The curvature at the texel (0.5 flat) through the mode; `blur` widens every radius.
    pub fn value(&self, t: &Texel, maps: &SetMaps, blur: f32) -> f32 {
        let total: f32 = self.weights.iter().sum();
        let c = if total <= 1e-6 {
            0.5
        } else {
            let sum: f32 = self.weights.iter().zip(RADII).filter(|(w, _)| **w > 0.0).map(|(w, r)| w * (maps.curvature(t.u, t.v, r + blur) - 0.5)).sum();
            0.5 + sum / total
        };
        match self.mode {
            2 => unit((c - 0.5) * 2.0),
            3 => unit((0.5 - c) * 2.0),
            4 => unit((c - 0.5).abs() * 2.0),
            _ => unit(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::TextureSet;

    #[test]
    fn flat_maps_and_neutral_settings() {
        let maps = SetMaps::of(&TextureSet::new("s", "S", (4, 4)));
        let none: Vec<(String, String)> = Vec::new();
        let p = Params::new(&none);
        let t = Texel::at(1, 1, 4, 4);
        assert_eq!(MultiScale::read(&p, "c").value(&t, &maps, 0.0), 0.5);
        assert!(TextureLayer::read(&p, "tex1", 0).is_none());
        assert_eq!(Finish::read(&p).apply(0.3), 0.3);
        let v = vec![("c_mode".to_string(), "2".to_string()), ("invert".to_string(), "1".to_string())];
        let p = Params::new(&v);
        assert_eq!(MultiScale::read(&p, "c").value(&t, &maps, 0.0), 0.0);
        assert_eq!(Finish::read(&p).apply(0.3), 0.7);
        assert_eq!(blur_fraction(2.56), 0.01);
    }
}
