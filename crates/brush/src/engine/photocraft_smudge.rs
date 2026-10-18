//! The adapter to PhotoCraft's smudge (`photocraft_paint::retouch::Smudge`, pinned; ADR 0007) as the core's
//! [`LocalKernel`] (PT-23): smudge drags and mixes what is painted along the stroke (blur is its own mode, ADR 0008
//! mc2-q-pt-06c). The stamp stage gathers the values under each dab onto the dab frame's grid; here that grid
//! becomes PhotoCraft's `Region` and the dab's coverage its `Footprint`, the kernel runs, and the grid takes the
//! result. PhotoCraft carries a patch of paint from dab to dab, centred on the dab and premultiplied where the
//! image has alpha (no dark fringes when smudging into transparent areas).

use materialcraft::stamp::{DabSample, LocalGrid, LocalKernel};
use photocraft_geom::{Point, Rect};
use photocraft_paint::Dab;
use photocraft_paint::retouch::{Footprint, Region, Smudge};

/// Smudge for one image of one stroke.
pub struct SmudgeKernel {
    strength: f32,
    /// How far the carried patch reaches: the stroke's largest dab, in frame units.
    diameter: f32,
    /// Made at the first dab, once it is known whether the image has alpha.
    inner: Option<Smudge>,
}

impl SmudgeKernel {
    /// A smudge that lays down and keeps `strength` (0 … 1) of the carried paint per dab, for dabs up to
    /// `diameter` frame units across.
    pub fn new(strength: f32, diameter: f32) -> SmudgeKernel {
        let strength = if strength.is_finite() { strength.clamp(0.0, 1.0) } else { 0.0 };
        let diameter = if diameter.is_finite() { diameter.clamp(1.0, 8192.0) } else { 1.0 };
        SmudgeKernel { strength, diameter, inner: None }
    }
}

impl LocalKernel for SmudgeKernel {
    fn dab(&mut self, grid: &mut LocalGrid, dab: &DabSample) {
        let (w, h) = (grid.width, grid.height);
        if w == 0 || h == 0 || grid.values.len() != w * h || dab.footprint.values.len() != w * h {
            return;
        }
        let (Ok(wi), Ok(hi)) = (i32::try_from(w), i32::try_from(h)) else { return };
        let rect = Rect::new(grid.x0, grid.y0, grid.x0.saturating_add(wi), grid.y0.saturating_add(hi));
        let (strength, diameter, alpha) = (self.strength, self.diameter, grid.has_alpha.then_some(3));
        let smudge = self.inner.get_or_insert_with(|| Smudge::new(strength, None, diameter).with_alpha(alpha));
        let mut region = Region { rect, ch: 4, data: grid.values.iter().flatten().copied().collect() };
        let center = Point::new(f64::from(dab.center[0]), f64::from(dab.center[1]));
        let footprint = Footprint { dab: Dab::round(center, dab.radius, 1.0), index: dab.index as usize, rect, cov: dab.footprint.values.clone() };
        smudge.dab(&mut region, &footprint);
        for (v, px) in grid.values.iter_mut().zip(region.data.chunks_exact(4)) {
            if let [r, g, b, a] = *px {
                *v = [r, g, b, a];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::stamp::Footprint as Coverage;

    fn grid(w: usize, h: usize, at: impl Fn(usize, usize) -> [f32; 4]) -> LocalGrid {
        LocalGrid { x0: 0, y0: 0, width: w, height: h, values: (0..w * h).map(|i| at(i % w, i / w)).collect(), on_surface: vec![true; w * h], has_alpha: true }
    }

    fn dab(cx: f32, w: usize, h: usize, index: u64) -> DabSample {
        let values = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                if ((x - cx).powi(2) + (y - 10.0).powi(2)).sqrt() <= 6.0 { 1.0 } else { 0.0 }
            })
            .collect();
        DabSample { center: [cx, 10.0], radius: 6.0, opacity: 1.0, index, footprint: Coverage::new(0, 0, w, h, values).unwrap() }
    }

    #[test]
    fn smudge_drags_red_into_white_and_runs_the_same_twice() {
        let run = || {
            let mut g = grid(60, 20, |x, _| if x < 20 { [1.0, 0.0, 0.0, 1.0] } else { [1.0; 4] });
            let mut k = SmudgeKernel::new(0.8, 12.0);
            for (i, cx) in (14..44).step_by(3).enumerate() {
                k.dab(&mut g, &dab(cx as f32, 60, 20, i as u64));
            }
            g
        };
        let g = run();
        let green = |x: usize| g.values[10 * 60 + x][1];
        assert!(green(24) < 0.6, "red was dragged past the edge: {}", green(24));
        assert!(green(24) < green(38), "fading along the stroke");
        assert_eq!(g.values[60 + 30], [1.0; 4], "off the stroke nothing changes");
        assert_eq!(run(), g, "deterministic");
    }

    #[test]
    fn empty_or_mismatched_grids_are_left_alone() {
        let mut k = SmudgeKernel::new(0.5, 10.0);
        let mut g = grid(0, 0, |_, _| [0.0; 4]);
        k.dab(&mut g, &dab(5.0, 0, 0, 0));
        let mut g = grid(4, 4, |_, _| [0.5; 4]);
        let before = g.clone();
        k.dab(&mut g, &dab(5.0, 2, 2, 0));
        assert_eq!(g, before);
    }
}
