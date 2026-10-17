//! Curvature from the high mesh: around each surface point, rays find the neighbouring surface at the sampling
//! radius, and the way its normals turn away from (convex) or towards (concave) the point gives the curvature.
//! 0.5 is flat, brighter convex, darker concave; the range comes from [`CurvatureRange`].
//!
//! Calibrated with parity: the neighbours are found on surfaces facing the point's side only (so other parts lying
//! on or crossing the surface don't show), and the automatic range maps nearly the strongest curvature of the map
//! (the 99.9th percentile) to black and white, which matched the user's bakes better than lower percentiles or a
//! fixed range.

use super::context::Ctx;
use super::high::Facing;
use super::rays::basis;
use super::settings::CurvatureRange;
use crate::v3::*;

/// Share of texels whose curvature lies inside the automatic range (the rest saturate).
const AUTO_PERCENTILE: f32 = 0.999;

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    let s = ctx.settings.curvature;
    let radius = s.radius.max(1e-6) * ctx.scale;
    let rays = ctx.rays_per_sample(s.rays.clamp(1, 256)).max(4);
    let raw = ctx.map([0.0; 4], |i, sample| {
        let (p, n) = sample.surface();
        let (t, b) = basis(n);
        let turn = ctx.turn(i, 17);
        let (mut sum, mut count) = (0.0f32, 0u32);
        for k in 0..rays {
            let phi = (k as f32 / rays as f32 + turn) * std::f32::consts::TAU;
            let dir = add(mul(t, phi.cos()), mul(b, phi.sin()));
            let origin = add(add(p, mul(dir, radius)), mul(n, 2.0 * radius));
            if let Some(hit) = ctx.high.trace(origin, mul(n, -1.0), 0.0, 4.0 * radius, Facing::Front) {
                sum += dot(sub(hit.normal, n), dir) / radius;
                count += 1;
            }
        }
        let k = if count > 0 { sum / count as f32 } else { 0.0 };
        [k, 0.0, 0.0, 1.0]
    });
    let full = match s.range {
        CurvatureRange::Fixed(k) => k.abs().max(1e-6),
        CurvatureRange::Auto => auto_range(&raw, ctx),
    };
    ctx.map([0.5, 0.5, 0.5, 1.0], |i, _| {
        let k = raw.get(i).map_or(0.0, |p| p[0]);
        let v = (0.5 + 0.5 * k / full).clamp(0.0, 1.0);
        [v, v, v, 1.0]
    })
}

/// The curvature magnitude below which [`AUTO_PERCENTILE`] of the covered texels lie.
fn auto_range(raw: &[[f32; 4]], ctx: &Ctx) -> f32 {
    let mut mags: Vec<f32> = raw
        .iter()
        .enumerate()
        .filter(|(i, _)| matches!(ctx.samples.texels.get(*i), Some(Some(_))))
        .map(|(_, p)| p[0].abs())
        .filter(|k| k.is_finite())
        .collect();
    if mags.is_empty() {
        return 1.0;
    }
    let at = ((mags.len() - 1) as f32 * AUTO_PERCENTILE) as usize;
    let (_, v, _) = mags.select_nth_unstable_by(at, f32::total_cmp);
    v.max(1e-6)
}
