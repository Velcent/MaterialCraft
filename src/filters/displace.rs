//! Filters that move texels along a noise ([`crate::noise::Fbm`], evaluated at the texel's UV on the texture
//! set, so the result does not depend on how the texture is cut into tiles).
//!
//! - Warp: every texel reads the input displaced by up to `intensity` (UV units, default 0.01); the
//!   displacement comes from two noises (x and y), or with `directional` (switch) from one noise along `angle`
//!   (turns). `scale` multiplies the noise's cell count (default 1, 4 cells), `seed` varies it.
//! - Slope blur: every texel combines the input along a path of `intensity` (UV units, default 0.01) down the
//!   slope of a noise (`scale`, `seed` as for warp): `mode` 0 averages, 1 keeps the minimum, 2 the maximum;
//!   `samples` (default 16) steps along the path.

use super::kernel::{AreaFilter, Ctx};
use super::plane::Plane;
use crate::noise::Fbm;
use crate::paint::effect::Params;

/// The noise both filters move texels along.
/// `octaves`: warps take some detail; slope directions stay smooth with few.
fn noise(p: &Params, seed_offset: u32, octaves: u32) -> Fbm {
    let cells = (4.0 * p.range("scale", 1.0, 0.1, 64.0)).round().max(1.0) as i32;
    Fbm::new(cells, cells, octaves, 0.5, p.seed("seed").wrapping_add(seed_offset), 0)
}

/// Runs `f(x, y)` for every texel at least `margin` texels inside the plane; the others keep the input.
fn inner(input: &Plane, margin: usize, mut f: impl FnMut(usize, usize) -> [f32; 4]) -> Vec<[f32; 4]> {
    let mut out = input.px.clone();
    for y in margin..input.h.saturating_sub(margin) {
        for x in margin..input.w.saturating_sub(margin) {
            let v = f(x, y);
            if let Some(o) = out.get_mut(y * input.w + x) {
                *o = v;
            }
        }
    }
    out
}

pub struct Warp;

pub struct WarpSettings {
    /// UV units.
    intensity: f32,
    x: Fbm,
    y: Fbm,
    /// Unit direction for directional warps.
    direction: Option<(f32, f32)>,
}

impl AreaFilter for Warp {
    type Settings = WarpSettings;

    fn settings(&self, p: &Params, _: &Ctx) -> WarpSettings {
        let turns = p.f("angle", 0.0) * std::f32::consts::TAU;
        WarpSettings {
            intensity: p.range("intensity", 0.01, 0.0, 0.25),
            x: noise(p, 0, 4),
            y: noise(p, 7919, 4),
            direction: p.on("directional", false).then(|| (turns.cos(), turns.sin())),
        }
    }

    fn radius(&self, s: &WarpSettings, ctx: &Ctx) -> u32 {
        ctx.texels(s.intensity).ceil() as u32 + 2
    }

    fn run(&self, s: &WarpSettings, ctx: &Ctx, input: &Plane) -> Vec<[f32; 4]> {
        let (w, h) = (ctx.size.0 as f32, ctx.size.1 as f32);
        let margin = self.radius(s, ctx) as usize;
        inner(input, margin, |x, y| {
            let (u, v) = input.uv(x, y, ctx.size);
            let a = (s.x.sample(u, v) - 0.5) * 2.0 * s.intensity;
            let (du, dv) = match s.direction {
                Some((c, si)) => (a * c, a * si),
                None => (a, (s.y.sample(u, v) - 0.5) * 2.0 * s.intensity),
            };
            input.bilinear(x as f32 + du * w, y as f32 + dv * h)
        })
    }
}

pub struct SlopeBlur;

pub struct SlopeSettings {
    intensity: f32,
    slope: Fbm,
    mode: i32,
    samples: u32,
}

impl AreaFilter for SlopeBlur {
    type Settings = SlopeSettings;

    fn settings(&self, p: &Params, _: &Ctx) -> SlopeSettings {
        SlopeSettings {
            intensity: p.range("intensity", 0.01, 0.0, 0.25),
            slope: noise(p, 0, 2),
            mode: p.int("mode", 0).clamp(0, 2),
            samples: p.int("samples", 16).clamp(2, 64) as u32,
        }
    }

    fn radius(&self, s: &SlopeSettings, ctx: &Ctx) -> u32 {
        ctx.texels(s.intensity).ceil() as u32 + 2
    }

    fn run(&self, s: &SlopeSettings, ctx: &Ctx, input: &Plane) -> Vec<[f32; 4]> {
        let size = ctx.size;
        let (du, dv) = (1.0 / size.0.max(1) as f32, 1.0 / size.1.max(1) as f32);
        let length = ctx.texels(s.intensity);
        let step = length / (s.samples - 1) as f32;
        let margin = self.radius(s, ctx) as usize;
        inner(input, margin, |x, y| {
            let (u, v) = input.uv(x, y, size);
            // downhill direction of the noise
            let gx = s.slope.sample(u + du, v) - s.slope.sample(u - du, v);
            let gy = s.slope.sample(u, v + dv) - s.slope.sample(u, v - dv);
            let len = (gx * gx + gy * gy).sqrt();
            let (dx, dy) = if len > 1e-9 { (-gx / len, -gy / len) } else { (0.0, 0.0) };
            let mut acc = match s.mode {
                1 => [f32::MAX; 4],
                2 => [f32::MIN; 4],
                _ => [0.0; 4],
            };
            for k in 0..s.samples {
                let t = k as f32 * step;
                let p = input.bilinear(x as f32 + dx * t, y as f32 + dy * t);
                for (a, q) in acc.iter_mut().zip(p) {
                    *a = match s.mode {
                        1 => a.min(q),
                        2 => a.max(q),
                        _ => *a + q / s.samples as f32,
                    };
                }
            }
            acc
        })
    }
}
