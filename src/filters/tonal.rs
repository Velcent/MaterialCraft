//! Per-texel tone filters: invert, height adjust, clamp, histogram scan and shift, contrast/luminosity. They act
//! on R, G and B (a mask's grey value is R) and keep the coverage. Colour channels are adjusted in their stored
//! (sRGB) encoding, as a picker shows them.

use super::colour::Display;
use super::kernel::{Ctx, PointFilter};
use crate::paint::Channel;
use crate::paint::effect::Params;

/// Applies `f` to R, G and B in display encoding.
fn each(ctx: &Ctx, v: [f32; 4], f: impl Fn(f32) -> f32) -> [f32; 4] {
    let d = Display(ctx.linear_light);
    let g = |x: f32| ctx.limit(d.decode(f(d.encode(x))));
    [g(v[0]), g(v[1]), g(v[2]), v[3]]
}

/// `invert` (switch, default on): 1 − v; on height, −v.
pub struct Invert;

impl PointFilter for Invert {
    type Settings = bool;

    fn settings(&self, p: &Params, _: &Ctx) -> bool {
        p.on("invert", true)
    }

    fn texel(&self, on: &bool, ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        if !*on {
            return v;
        }
        if ctx.channel() == Some(Channel::Height) { [-v[0], -v[1], -v[2], v[3]] } else { each(ctx, v, |x| 1.0 - x) }
    }
}

/// Height adjust: `invert` (switch) negates, then × `multiplier` (default 1), then + `offset` (default 0).
pub struct HeightAdjust;

pub struct HeightSettings {
    invert: bool,
    multiplier: f32,
    offset: f32,
}

impl PointFilter for HeightAdjust {
    type Settings = HeightSettings;

    fn settings(&self, p: &Params, _: &Ctx) -> HeightSettings {
        HeightSettings { invert: p.on("invert", false), multiplier: p.range("multiplier", 1.0, -100.0, 100.0), offset: p.range("offset", 0.0, -100.0, 100.0) }
    }

    fn texel(&self, s: &HeightSettings, ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        let sign = if s.invert { -1.0 } else { 1.0 };
        each(ctx, v, |x| x * sign * s.multiplier + s.offset)
    }
}

/// Clamp to `min` .. `max` (defaults 0 and 1).
pub struct Clamp;

impl PointFilter for Clamp {
    type Settings = (f32, f32);

    fn settings(&self, p: &Params, _: &Ctx) -> (f32, f32) {
        let (a, b) = (p.f("min", 0.0), p.f("max", 1.0));
        (a.min(b), a.max(b))
    }

    fn texel(&self, s: &(f32, f32), ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        each(ctx, v, |x| x.clamp(s.0, s.1))
    }
}

/// Histogram scan: a ramp centred at `1 − position` (`position` 0..1, default 0.5) whose width shrinks with
/// `contrast` (0..1, 0 = the identity at position 0.5). `invert_position` mirrors the position.
pub struct HistogramScan;

impl PointFilter for HistogramScan {
    type Settings = (f32, f32);

    fn settings(&self, p: &Params, _: &Ctx) -> (f32, f32) {
        let position = p.range("position", 0.5, 0.0, 1.0);
        let position = if p.on("invert_position", false) { 1.0 - position } else { position };
        (1.0 - position, (1.0 - p.range("contrast", 0.0, 0.0, 1.0)).max(1e-3))
    }

    fn texel(&self, s: &(f32, f32), ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        each(ctx, v, |x| ((x - s.0) / s.1 + 0.5).clamp(0.0, 1.0))
    }
}

/// Histogram shift: adds `position` (0..1) and wraps around 1.
pub struct HistogramShift;

impl PointFilter for HistogramShift {
    type Settings = f32;

    fn settings(&self, p: &Params, _: &Ctx) -> f32 {
        p.range("position", 0.0, -1.0, 1.0)
    }

    fn texel(&self, s: &f32, ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        each(ctx, v, |x| {
            let y = (x + s).rem_euclid(1.0);
            if x >= 1.0 && *s == 0.0 { 1.0 } else { y }
        })
    }
}

/// Contrast around mid grey (`contrast` −1..1, 0 neutral) and a brightness offset (`luminosity` −1..1).
pub struct ContrastLuminosity;

impl PointFilter for ContrastLuminosity {
    type Settings = (f32, f32);

    fn settings(&self, p: &Params, _: &Ctx) -> (f32, f32) {
        let c = p.range("contrast", 0.0, -1.0, 1.0);
        // contrast 1 is a hard step, −1 flat grey
        let gain = if c >= 0.0 { 1.0 / (1.0 - c).max(1e-3) } else { 1.0 + c };
        (gain, p.range("luminosity", 0.0, -1.0, 1.0))
    }

    fn texel(&self, s: &(f32, f32), ctx: &Ctx, v: [f32; 4], _: (f32, f32)) -> [f32; 4] {
        each(ctx, v, |x| (x - 0.5) * s.0 + 0.5 + s.1)
    }
}
