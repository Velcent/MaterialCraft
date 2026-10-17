//! The two shapes a filter of ours takes, and their adapters to the paint model's [`FilterKernel`]:
//!
//! - [`PointFilter`] maps each texel on its own (colour and tone adjustments); [`Point`] runs it.
//! - [`AreaFilter`] reads a neighbourhood (blur, sharpen, bevel, warp …); [`Area`] runs it once per UV island
//!   on a tile padded so that the island's texels see only their own island ([`super::islands`]).
//!
//! Sizes in parameters are in UV units (a fraction of the texture's larger side), so a filter looks the same
//! at every resolution; [`Ctx::texels`] converts them.

use super::islands;
use super::plane::Plane;
use crate::paint::effect::{EvalRequest, EvalTarget, FilterInput, FilterKernel, Params};
use crate::paint::{Channel, ColorSpace};

/// The largest neighbourhood radius a filter may ask for, in texels.
pub const MAX_RADIUS: u32 = 512;

/// What a filter runs for: the target, the texture set's size and the channel's working space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ctx {
    pub target: EvalTarget,
    pub size: (u32, u32),
    /// Values are linear light decoded from sRGB (colour channels); colour adjustments encode them first.
    pub linear_light: bool,
}

impl Ctx {
    pub fn of(request: &EvalRequest) -> Ctx {
        let linear_light = match request.target {
            EvalTarget::Channel(c) => request.set.channel_spec(c).map_or(c.default_color_space(), |s| s.color_space) == ColorSpace::Srgb,
            EvalTarget::Mask => false,
        };
        Ctx { target: request.target, size: request.set.resolution, linear_light }
    }

    /// A length in UV units as texels of the larger side.
    pub fn texels(&self, uv: f32) -> f32 {
        let t = uv * self.size.0.max(self.size.1) as f32;
        if t.is_finite() { t } else { 0.0 }
    }

    /// The channel, unless the target is a mask.
    pub fn channel(&self) -> Option<Channel> {
        match self.target {
            EvalTarget::Channel(c) => Some(c),
            EvalTarget::Mask => None,
        }
    }

    /// Height is signed and unbounded; every other target keeps values in 0..1.
    pub fn bounded(&self) -> bool {
        self.channel() != Some(Channel::Height)
    }

    /// `v` limited to the target's range.
    pub fn limit(&self, v: f32) -> f32 {
        let v = if v.is_finite() { v } else { 0.0 };
        if self.bounded() { v.clamp(0.0, 1.0) } else { v }
    }
}

/// A filter that maps each texel on its own.
pub trait PointFilter: Send + Sync {
    /// The parameters read once per request.
    type Settings;

    fn settings(&self, p: &Params, ctx: &Ctx) -> Self::Settings;

    /// The new value of one texel (straight RGBA, alpha = coverage) at UV `uv`.
    fn texel(&self, s: &Self::Settings, ctx: &Ctx, value: [f32; 4], uv: (f32, f32)) -> [f32; 4];
}

/// A filter that reads a neighbourhood.
pub trait AreaFilter: Send + Sync {
    type Settings;

    fn settings(&self, p: &Params, ctx: &Ctx) -> Self::Settings;

    /// How far (texels) the result of a texel depends on its neighbours.
    fn radius(&self, s: &Self::Settings, ctx: &Ctx) -> u32;

    /// The filtered plane: as many texels as `input`, same placement. Texels within the radius of the plane's
    /// edges may be wrong; only the inner ones are used.
    fn run(&self, s: &Self::Settings, ctx: &Ctx, input: &Plane) -> Vec<[f32; 4]>;
}

/// A [`PointFilter`] as a paint-model filter.
pub struct Point<F>(pub F);

impl<F: PointFilter> FilterKernel for Point<F> {
    fn apply(&self, request: &EvalRequest, input: &FilterInput, out: &mut [[f32; 4]]) -> Result<(), String> {
        let ctx = Ctx::of(request);
        let s = self.0.settings(&Params::new(request.params), &ctx);
        let (r, i) = (request.rect, input.rect);
        if r.x < i.x || r.y < i.y || r.x + r.w > i.x + i.w || r.y + r.h > i.y + i.h || input.pixels.len() < i.area() || out.len() < r.area() {
            return Err("filter input does not cover the output".into());
        }
        let size = ctx.size;
        for (k, o) in out.iter_mut().enumerate().take(r.area()) {
            let (x, y) = (r.x + k as u32 % r.w.max(1), r.y + k as u32 / r.w.max(1));
            let v = input.pixels.get(((y - i.y) * i.w + (x - i.x)) as usize).copied().unwrap_or([0.0; 4]);
            let uv = ((x as f32 + 0.5) / size.0.max(1) as f32, (y as f32 + 0.5) / size.1.max(1) as f32);
            *o = self.0.texel(&s, &ctx, v, uv);
        }
        Ok(())
    }
}

/// An [`AreaFilter`] as a paint-model filter, run per UV island.
pub struct Area<F>(pub F);

impl<F: AreaFilter> Area<F> {
    fn radius_of(&self, request: &EvalRequest) -> (Ctx, F::Settings, u32) {
        let ctx = Ctx::of(request);
        let s = self.0.settings(&Params::new(request.params), &ctx);
        let r = self.0.radius(&s, &ctx).min(MAX_RADIUS);
        (ctx, s, r)
    }
}

impl<F: AreaFilter> FilterKernel for Area<F> {
    fn radius(&self, request: &EvalRequest) -> u32 {
        self.radius_of(request).2
    }

    fn apply(&self, request: &EvalRequest, input: &FilterInput, out: &mut [[f32; 4]]) -> Result<(), String> {
        let (ctx, s, radius) = self.radius_of(request);
        islands::run(request, input, radius, out, |plane| self.0.run(&s, &ctx, plane))
    }
}
