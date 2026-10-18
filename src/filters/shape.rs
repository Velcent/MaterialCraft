//! Filters that reshape a mask around its edge (the 0.5 level of the grey value), from the distance to the edge
//! (the designer's `ops::distance_field`).
//!
//! - Bevel: `distance` (UV units, signed): positive ramps the shapes up from their edge inwards over that
//!   distance, negative grows a ramp outwards from the edge; `smoothing` (sigma in UV units) blurs the result;
//!   `round` (switch) gives a rounded instead of a straight profile.
//! - Mask outline: a band along the edge, `width` UV units wide, `position` 0 inside the shapes, 1 centred on the
//!   edge (default), 2 outside; `curve` (−1..1, 0 flat) thins (negative) or fills (positive) the band's profile;
//!   `blur` (sigma in UV units) softens it.

use super::gauss::{blur, extent, reach};
use super::kernel::{AreaFilter, Ctx};
use super::plane::Plane;
use crate::ops::distance_field;
use crate::paint::effect::Params;

/// Distance (texels) of every texel to the edge: from inside texels to the nearest outside one, and the other way.
struct Edge {
    inside: Vec<bool>,
    to_out: Vec<f32>,
    to_in: Vec<f32>,
}

impl Edge {
    fn of(p: &Plane) -> Edge {
        let inside: Vec<bool> = p.px.iter().map(|v| v[0] >= 0.5).collect();
        let outside: Vec<bool> = inside.iter().map(|i| !i).collect();
        Edge { to_out: distance_field(&inside, p.w, p.h), to_in: distance_field(&outside, p.w, p.h), inside }
    }

    /// Signed distance to the edge, positive inside, texel centres half a texel off the edge.
    fn signed(&self, i: usize) -> f32 {
        match (self.inside.get(i), self.to_out.get(i), self.to_in.get(i)) {
            (Some(true), Some(d), _) => d - 0.5,
            (Some(false), _, Some(d)) => 0.5 - d,
            _ => 0.0,
        }
    }
}

fn round_profile(t: f32, round: bool) -> f32 {
    if round { (1.0 - (1.0 - t) * (1.0 - t)).max(0.0).sqrt() } else { t }
}

/// How far (texels) a texel's grey value reads when it stays the same for every edge distance of `saturation`
/// texels or more, blurred by `sigma`. The distances are chamfer path lengths ([`distance_field`]: a fixed
/// sweep pattern, each step at least one texel), so a texel `k` steps away (Chebyshev) only enters a distance
/// of `k` or more: one at `ceil(saturation)` or farther can't change a value below the saturation.
fn edge_extent(saturation: f32, sigma: f32) -> u32 {
    let s = if saturation.is_finite() { saturation.max(0.0).ceil().min(u32::MAX as f32) as u32 } else { u32::MAX };
    s.saturating_add(extent(sigma))
}

/// Grey values with the input's coverage, blurred by `sigma` texels.
fn finish(input: &Plane, grey: Vec<f32>, sigma: f32) -> Vec<[f32; 4]> {
    let px = input.with_px(grey.iter().zip(&input.px).map(|(g, v)| [*g, *g, *g, v[3]]).collect());
    if sigma > 0.05 { blur(&px, sigma) } else { px.px }
}

pub struct Bevel;

pub struct BevelSettings {
    /// Texels, signed.
    distance: f32,
    sigma: f32,
    round: bool,
}

impl AreaFilter for Bevel {
    type Settings = BevelSettings;

    fn settings(&self, p: &Params, ctx: &Ctx) -> BevelSettings {
        BevelSettings {
            distance: ctx.texels(p.range("distance", 0.02, -0.25, 0.25)),
            sigma: ctx.texels(p.range("smoothing", 0.0, 0.0, 0.1)),
            round: p.on("round", false),
        }
    }

    fn radius(&self, s: &BevelSettings, _: &Ctx) -> u32 {
        s.distance.abs().ceil() as u32 + reach(s.sigma) + 2
    }

    /// The ramp is flat once the edge is `max(|distance|, 1) + 0.5` texels away (see [`Self::run`]).
    fn extent(&self, s: &BevelSettings, _: &Ctx) -> u32 {
        edge_extent(s.distance.abs().max(1.0) + 0.5, s.sigma)
    }

    fn run(&self, s: &BevelSettings, _: &Ctx, input: &Plane) -> Vec<[f32; 4]> {
        let edge = Edge::of(input);
        let d = s.distance.abs().max(1.0);
        let grey = (0..input.px.len())
            .map(|i| {
                let sd = edge.signed(i);
                let t = if s.distance >= 0.0 { sd / d } else { 1.0 + sd / d };
                round_profile(t.clamp(0.0, 1.0), s.round)
            })
            .collect();
        finish(input, grey, s.sigma)
    }
}

pub struct MaskOutline;

pub struct OutlineSettings {
    /// The band in signed texels from the edge (positive inside).
    from: f32,
    to: f32,
    /// Exponent of the band's tent profile; `None` for a flat band.
    curve: Option<f32>,
    sigma: f32,
}

impl AreaFilter for MaskOutline {
    type Settings = OutlineSettings;

    fn settings(&self, p: &Params, ctx: &Ctx) -> OutlineSettings {
        let w = ctx.texels(p.range("width", 0.02, 0.0, 0.25)).max(1.0);
        let (from, to) = match p.int("position", 1) {
            0 => (0.0, w),
            2 => (-w, 0.0),
            _ => (-w / 2.0, w / 2.0),
        };
        let c = p.range("curve", 0.0, -1.0, 1.0);
        OutlineSettings { from, to, curve: (c != 0.0).then(|| (-2.0 * c).exp2()), sigma: ctx.texels(p.range("blur", 0.0, 0.0, 0.1)) }
    }

    fn radius(&self, s: &OutlineSettings, _: &Ctx) -> u32 {
        s.from.abs().max(s.to.abs()).ceil() as u32 + reach(s.sigma) + 2
    }

    /// The band is empty once the edge is one texel beyond its far side: `max(to, −from) + 1` texels.
    fn extent(&self, s: &OutlineSettings, _: &Ctx) -> u32 {
        edge_extent(s.to.max(-s.from) + 1.0, s.sigma)
    }

    fn run(&self, s: &OutlineSettings, _: &Ctx, input: &Plane) -> Vec<[f32; 4]> {
        let edge = Edge::of(input);
        let span = (s.to - s.from).max(1e-3);
        let grey = (0..input.px.len())
            .map(|i| {
                let sd = edge.signed(i);
                // half a texel of anti-aliasing at both sides of the band
                let cover = ((sd - s.from).min(s.to - sd) + 0.5).clamp(0.0, 1.0);
                let shape = match s.curve {
                    Some(e) => {
                        let t = ((sd - s.from) / span).clamp(0.0, 1.0);
                        (1.0 - (2.0 * t - 1.0).abs()).powf(e)
                    }
                    None => 1.0,
                };
                cover * shape
            })
            .collect();
        finish(input, grey, s.sigma)
    }
}
