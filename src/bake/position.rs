//! Position map: where the surface lies, normalised into 0..1 over a bounding box ([`PositionSettings`]).
//!
//! Measured on the user's own bakes (parity, 2026-10-17): Painter's default output is the uniform mode, centred on
//! the bounding box with the longest side spanning 0..1, over the whole scene or (with its per-set option) over the
//! texture set's own parts.

use super::context::Ctx;
use super::settings::{PositionBounds, PositionMode, PositionSettings};
use crate::mesh::Bounds;
use crate::v3::*;

pub(crate) fn bake(ctx: &Ctx) -> Vec<[f32; 4]> {
    let settings = ctx.settings.position;
    let bounds = match settings.bounds {
        PositionBounds::Scene => ctx.scene_bounds,
        PositionBounds::Set => ctx.set_bounds,
        PositionBounds::High => {
            let (min, max) = ctx.high.bounds();
            Some(Bounds { min, max })
        }
    };
    let Some(bounds) = bounds else {
        return ctx.map([0.0, 0.0, 0.0, 1.0], |_, _| [0.5, 0.5, 0.5, 1.0]);
    };
    let normaliser = Normaliser::new(bounds, settings);
    ctx.map([0.0, 0.0, 0.0, 1.0], |_, s| {
        let p = normaliser.apply(s.surface().0);
        [p[0], p[1], p[2], 1.0]
    })
}

/// `(p - centre) * scale + 0.5` per axis.
struct Normaliser {
    centre: V,
    scale: V,
}

impl Normaliser {
    fn new(b: Bounds, s: PositionSettings) -> Normaliser {
        let size = b.size();
        let inv = |e: f32| if e > 1e-9 { 1.0 / e } else { 0.0 };
        let scale = match s.mode {
            PositionMode::Uniform => [inv(size[0].max(size[1]).max(size[2])); 3],
            PositionMode::PerAxis => size.map(inv),
        };
        Normaliser { centre: b.centre(), scale }
    }

    fn apply(&self, p: V) -> V {
        let mut out = [0.0; 3];
        for (((o, p), c), s) in out.iter_mut().zip(p).zip(self.centre).zip(self.scale) {
            *o = ((p - c) * s + 0.5).clamp(0.0, 1.0);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_and_per_axis() {
        let b = Bounds { min: [-1.0, -0.5, 0.0], max: [1.0, 0.5, 0.2] };
        let u = Normaliser::new(b, PositionSettings { mode: PositionMode::Uniform, bounds: PositionBounds::Scene });
        assert_eq!(u.apply([1.0, 0.5, 0.1]), [1.0, 0.75, 0.5]);
        let a = Normaliser::new(b, PositionSettings { mode: PositionMode::PerAxis, bounds: PositionBounds::Scene });
        let p = a.apply([1.0, 0.5, 0.2]);
        assert!(p.iter().all(|v| (v - 1.0).abs() < 1e-6), "{p:?}");
    }
}
