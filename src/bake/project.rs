//! Projecting the low surface onto the high mesh: one ray per (sub-)texel from the cage (the low surface pushed out
//! by the frontal distance) back along the ray direction, at most frontal + rear distance far. The first high surface
//! it meets is what the texel shows. [`Samples`] keeps the result and is what every baker reads.

use super::cancel::CancelToken;
use super::high::{Facing, HighHit, HighScene};
use super::low::{LowSurface, LowTexel};
use crate::tex::par_map;
use crate::v3::*;

/// One (sub-)texel: the low surface under it and, if a ray found it, the high surface.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Sample {
    pub low: LowTexel,
    pub hit: Option<HighHit>,
}

impl Sample {
    /// The surface the maps describe: the high hit, else the low surface itself.
    pub fn surface(&self) -> (V, V) {
        match &self.hit {
            Some(h) => (h.position, h.normal),
            None => (self.low.position, self.low.normal),
        }
    }
}

/// The projected texels of one bake, `width` x `width`, row-major.
pub(crate) struct Samples {
    pub width: usize,
    pub texels: Vec<Option<Sample>>,
}

/// The ray distances in scene units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reach {
    pub front: f32,
    pub rear: f32,
    pub facing: Facing,
}

impl Samples {
    pub fn project(low: &LowSurface, high: &HighScene, reach: Reach, cancel: &CancelToken) -> Samples {
        let width = low.size();
        let texels = par_map(width * width, |i| {
            if cancel.is_cancelled() {
                return None;
            }
            let low = low.texel(i)?;
            Some(Sample { low, hit: cast(high, &low, reach) })
        });
        Samples { width, texels }
    }

    /// Share of covered texels whose ray found the high mesh.
    pub fn hit_share(&self) -> f32 {
        let (covered, hit) = self.texels.iter().flatten().fold((0usize, 0usize), |(c, h), s| (c + 1, h + usize::from(s.hit.is_some())));
        if covered == 0 { 0.0 } else { hit as f32 / covered as f32 }
    }
}

fn cast(high: &HighScene, low: &LowTexel, reach: Reach) -> Option<HighHit> {
    let origin = add(low.position, mul(low.ray, reach.front));
    let down = mul(low.ray, -1.0);
    high.trace(origin, down, 0.0, reach.front + reach.rear, reach.facing)
}
