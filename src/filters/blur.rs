//! Blur and sharpen.
//!
//! - Blur: a gaussian of `radius` (sigma in UV units, default 0.002, about 4 texels at 2K).
//! - Sharpen: unsharp masking, value + (value − blurred) × `amount` (default 1), the blur of `radius` (sigma in
//!   UV units, default 1/2048, at least half a texel).

use super::gauss::{blur, extent, reach};
use super::kernel::{AreaFilter, Ctx};
use super::plane::Plane;
use crate::paint::effect::Params;

pub struct Blur;

impl AreaFilter for Blur {
    /// Sigma in texels.
    type Settings = f32;

    fn settings(&self, p: &Params, ctx: &Ctx) -> f32 {
        ctx.texels(p.range("radius", 0.002, 0.0, 0.25))
    }

    fn radius(&self, sigma: &f32, _: &Ctx) -> u32 {
        reach(*sigma)
    }

    fn extent(&self, sigma: &f32, _: &Ctx) -> u32 {
        extent(*sigma)
    }

    fn run(&self, sigma: &f32, _: &Ctx, input: &Plane) -> Vec<[f32; 4]> {
        blur(input, *sigma)
    }
}

pub struct Sharpen;

pub struct SharpenSettings {
    sigma: f32,
    amount: f32,
}

impl AreaFilter for Sharpen {
    type Settings = SharpenSettings;

    fn settings(&self, p: &Params, ctx: &Ctx) -> SharpenSettings {
        let sigma = ctx.texels(p.range("radius", 1.0 / 2048.0, 0.0, 0.05)).max(0.5);
        SharpenSettings { sigma, amount: p.range("amount", 1.0, 0.0, 50.0) }
    }

    fn radius(&self, s: &SharpenSettings, _: &Ctx) -> u32 {
        reach(s.sigma)
    }

    /// The blur's; the unsharp step is per texel.
    fn extent(&self, s: &SharpenSettings, _: &Ctx) -> u32 {
        extent(s.sigma)
    }

    fn run(&self, s: &SharpenSettings, ctx: &Ctx, input: &Plane) -> Vec<[f32; 4]> {
        let blurred = blur(input, s.sigma);
        input
            .px
            .iter()
            .zip(blurred)
            .map(|(v, b)| {
                let f = |x: f32, y: f32| ctx.limit(x + (x - y) * s.amount);
                [f(v[0], b[0]), f(v[1], b[1]), f(v[2], b[2]), v[3]]
            })
            .collect()
    }
}
