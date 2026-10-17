//! Thickness: how far rays cast into the mesh from the high surface travel before they leave it again, averaged
//! over a cone around the inward normal; rays that find nothing within the distance count as the full distance.
//! 1 = thick (or open), 0 = thin.
//!
//! With "maximise range" the map is stretched so its thinnest texel is black and its thickest white: measured
//! against the user's bakes, stretching both ends matched clearly better than scaling the maximum alone.

use super::ao::SELF_OFFSET;
use super::context::Ctx;
use super::high::Facing;
use super::rays::Cone;
use crate::v3::*;

/// What the thickness distance is relative to, as a share of the bake's scale (the bounding-box diagonal): half the
/// diagonal took the feature-cover median from 24.0 to 16.6 steps (0.35: 22.2, 1: 24.0, 2: 39.7; a tone curve on top
/// only made it worse).
const DISTANCE_UNIT: f32 = 0.5;

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    let s = ctx.settings.thickness;
    let reach = s.max_dist.max(1e-6) * ctx.scale * DISTANCE_UNIT;
    let offset = SELF_OFFSET * ctx.scale;
    let rays = ctx.rays_per_sample(s.rays.clamp(1, 1024));
    let mut out = ctx.map([0.0, 0.0, 0.0, 1.0], |i, sample| {
        let (p, n) = sample.surface();
        let inward = mul(n, -1.0);
        let origin = add(p, mul(inward, offset));
        let cone = Cone::new(inward, rays, 180.0, s.distribution, ctx.turn(i, 13));
        let mut sum = 0.0;
        for k in 0..cone.count() {
            let (d, _) = cone.direction(k);
            sum += ctx.high.trace(origin, d, 0.0, reach, Facing::Any).map_or(reach, |h| h.t);
        }
        let v = sum / (cone.count() as f32 * reach);
        [v, v, v, 1.0]
    });
    if s.maximise_range {
        stretch(&mut out, ctx);
    }
    out
}

/// Stretches covered texels so the smallest value becomes 0 and the largest 1.
fn stretch(out: &mut [[f32; 4]], ctx: &Ctx) {
    let covered = |i: usize| matches!(ctx.samples.texels.get(i), Some(Some(_)));
    let (min, max) = out.iter().enumerate().filter(|(i, _)| covered(*i)).fold((f32::MAX, f32::MIN), |(lo, hi), (_, p)| (lo.min(p[0]), hi.max(p[0])));
    if max - min <= 1e-6 {
        return;
    }
    for (i, p) in out.iter_mut().enumerate() {
        if covered(i) {
            let v = ((p[0] - min) / (max - min)).clamp(0.0, 1.0);
            *p = [v, v, v, 1.0];
        }
    }
}
