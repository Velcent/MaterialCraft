//! The HSL filter, perceptual: hue, chroma and lightness are changed in OkLCh (the designer's
//! `ops::rgb_to_lch` / `ops::lch_to_rgb`), so equal steps look equally strong across hues.
//!
//! Parameters (all 0 = unchanged): `hue` (turns, −0.5..0.5), `saturation` (−1..1: −1 grey, 1 about three times
//! the chroma), `lightness` (−1..1: −1 black, 1 white). On a mask (grey) only the lightness applies.

use super::kernel::{Ctx, PointFilter};
use crate::ops::{lch_to_rgb, rgb_to_lch};
use crate::paint::effect::Params;
use crate::paint::working_space::{linear_to_srgb, srgb_to_linear};

/// The encoding colour adjustments work in: values of a linear-light channel are encoded to sRGB first.
#[derive(Debug, Clone, Copy)]
pub struct Display(pub bool);

impl Display {
    pub fn encode(self, v: f32) -> f32 {
        if self.0 { linear_to_srgb(v.max(0.0)) } else { v }
    }

    pub fn decode(self, v: f32) -> f32 {
        if self.0 { srgb_to_linear(v) } else { v }
    }
}

pub struct Hsl;

pub struct HslSettings {
    hue: f32,
    saturation: f32,
    lightness: f32,
}

/// Lightness moved towards black (negative) or white (positive) by `amount`.
fn lighten(l: f32, amount: f32) -> f32 {
    if amount >= 0.0 { l + (1.0 - l) * amount } else { l * (1.0 + amount) }
}

impl PointFilter for Hsl {
    type Settings = HslSettings;

    fn settings(&self, p: &Params, _: &Ctx) -> HslSettings {
        HslSettings { hue: p.range("hue", 0.0, -1.0, 1.0), saturation: p.range("saturation", 0.0, -1.0, 1.0), lightness: p.range("lightness", 0.0, -1.0, 1.0) }
    }

    fn texel(&self, s: &HslSettings, ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        let d = Display(ctx.linear_light);
        if ctx.channel().is_none_or(|c| !c.is_color()) {
            let g = ctx.limit(d.decode(lighten(d.encode(v[0]).clamp(0.0, 1.0), s.lightness)));
            return [g, g, g, v[3]];
        }
        let rgb = [d.encode(v[0]), d.encode(v[1]), d.encode(v[2])].map(|x| x.clamp(0.0, 1.0));
        let [l, c, h] = rgb_to_lch(rgb);
        let gain = if s.saturation >= 0.0 { 1.0 + 2.0 * s.saturation } else { 1.0 + s.saturation };
        let out = lch_to_rgb([lighten(l, s.lightness).clamp(0.0, 1.0), c * gain, (h + s.hue).rem_euclid(1.0)]);
        let f = |x: f32| ctx.limit(d.decode(x));
        [f(out[0]), f(out[1]), f(out[2]), v[3]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Channel;
    use crate::paint::effect::EvalTarget;

    fn ctx(target: EvalTarget, linear_light: bool) -> Ctx {
        Ctx { target, size: (8, 8), linear_light }
    }

    fn params(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn neutral_settings_keep_colours_and_lightness_moves_grey() {
        let c = ctx(EvalTarget::Channel(Channel::BaseColor), true);
        let p = params(&[]);
        let s = Hsl.settings(&Params::new(&p), &c);
        let v = [0.2, 0.5, 0.05, 0.7];
        let o = Hsl.texel(&s, &c, v, (0.0, 0.0));
        assert!(v.iter().zip(o).all(|(a, b)| (a - b).abs() < 2e-3), "{o:?}");
        let p = params(&[("saturation", "-1")]);
        let o = Hsl.texel(&Hsl.settings(&Params::new(&p), &c), &c, v, (0.0, 0.0));
        assert!((o[0] - o[1]).abs() < 2e-3 && (o[1] - o[2]).abs() < 2e-3);
        let m = ctx(EvalTarget::Mask, false);
        let p = params(&[("lightness", "0.5")]);
        let o = Hsl.texel(&Hsl.settings(&Params::new(&p), &m), &m, [0.5, 0.5, 0.5, 1.0], (0.0, 0.0));
        assert!((o[0] - 0.75).abs() < 1e-5);
    }
}
