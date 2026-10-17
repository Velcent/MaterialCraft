//! Ambient occlusion: how much of the hemisphere above the high surface the high mesh hides, 1 open, 0 closed.
//! Rays leave the surface in a cone around its normal; each hit counts by its distance ([`Attenuation`]).

use super::context::Ctx;
use super::high::Facing;
use super::rays::Cone;
use super::settings::{Attenuation, RayDistribution};
use crate::v3::*;

/// Origin offset along the normal (relative), against the surface hitting itself.
pub(crate) const SELF_OFFSET: f32 = 1e-4;

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    let s = ctx.settings;
    let reach = s.ao_dist.max(1e-6) * ctx.scale;
    let near = (s.ao.min_dist.max(0.0) * ctx.scale).min(reach);
    let offset = SELF_OFFSET * ctx.scale;
    let rays = ctx.rays_per_sample(s.ao_rays.clamp(1, 1024));
    ctx.map([1.0; 4], |i, sample| {
        let (p, n) = sample.surface();
        let origin = add(p, mul(n, offset));
        let cone = Cone::new(n, rays, s.ao.spread, s.ao.distribution, ctx.turn(i, 7));
        let (mut hidden, mut total) = (0.0f32, 0.0f32);
        for k in 0..cone.count() {
            let (d, cos) = cone.direction(k);
            // uniform rays carry the cosine weight themselves; cosine-distributed ones already have it
            let weight = if s.ao.distribution == RayDistribution::Uniform { cos } else { 1.0 };
            total += weight;
            if let Some(hit) = ctx.high.trace(origin, d, near, reach, Facing::Any) {
                hidden += weight * falloff(s.ao.attenuation, hit.t / reach);
            }
        }
        let v = if total > 0.0 { 1.0 - hidden / total } else { 1.0 };
        [v, v, v, 1.0]
    })
}

/// How much an occluder at `x` = distance / reach (0..1) counts.
pub(crate) fn falloff(a: Attenuation, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    match a {
        Attenuation::None => 1.0,
        Attenuation::Linear => 1.0 - x,
        Attenuation::Smooth => {
            let s = x * x * (3.0 - 2.0 * x);
            1.0 - s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloffs() {
        for a in [Attenuation::None, Attenuation::Linear, Attenuation::Smooth] {
            assert_eq!(falloff(a, 0.0), 1.0);
            assert!(falloff(a, 0.5) <= 1.0 && falloff(a, 0.5) > 0.0);
        }
        assert_eq!(falloff(Attenuation::Linear, 1.0), 0.0);
        assert_eq!(falloff(Attenuation::Smooth, 1.0), 0.0);
        assert_eq!(falloff(Attenuation::None, 1.0), 1.0);
    }
}
