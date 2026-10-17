//! What the native effect kinds compute per value: [`Levels::apply`], [`ColorSelection::weight`] and the
//! triangle lookup of a polygon fill's picks ([`PickLookup`]). The compositor calls these per texel; they know
//! nothing about stacks.

use super::kinds::{ColorSelection, Levels, Selection};
use std::collections::HashSet;

impl Levels {
    /// One component through the levels: the input range mapped to 0..1 (clamped), the gamma curve
    /// (`t^(1/gamma)`, so a gamma above 1 brightens the midtones), then mapped to the output range (which may
    /// be inverted, min above max). With `clamp` the result is kept within 0..1.
    pub fn component(&self, value: f32, i: usize) -> f32 {
        let at = |a: [f32; 3]| a.get(i).or(a.first()).copied().unwrap_or(0.0);
        let (lo, hi, gamma) = (at(self.input_min), at(self.input_max), at(self.gamma));
        let range = hi - lo;
        let t = match range.abs() > 1e-6 {
            true => (value - lo) / range,
            false if value >= lo => 1.0,
            false => 0.0,
        };
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        let t = match gamma.is_finite() && gamma > 1e-6 {
            true => t.powf(1.0 / gamma),
            false => t,
        };
        let (out_lo, out_hi) = (at(self.output_min), at(self.output_max));
        let v = out_lo + t * (out_hi - out_lo);
        if self.clamp { v.clamp(0.0, 1.0) } else { v }
    }

    /// RGB through the levels per component; alpha (coverage) is kept.
    pub fn apply(&self, v: [f32; 4]) -> [f32; 4] {
        [self.component(v[0], 0), self.component(v[1], 1), self.component(v[2], 2), v[3]]
    }

    /// A grey value through the first component's levels.
    pub fn apply_grey(&self, v: f32) -> f32 {
        self.component(v, 0)
    }

    /// True when the levels change nothing.
    pub fn is_identity(&self) -> bool {
        *self == Levels { clamp: self.clamp, ..Levels::default() }
    }
}

impl ColorSelection {
    /// How much a map colour is selected, 0..1: 1 within `tolerance × hardness` of a picked colour (largest
    /// component difference), falling off smoothly to 0 at `tolerance`; the best picked colour wins.
    pub fn weight(&self, rgb: [f32; 3]) -> f32 {
        let tolerance = if self.tolerance.is_finite() { self.tolerance.max(0.0) } else { 0.0 };
        let hardness = if self.hardness.is_finite() { self.hardness.clamp(0.0, 1.0) } else { 1.0 };
        let inner = tolerance * hardness;
        let mut best = 0.0f32;
        for c in &self.colors {
            let d = (0..3).map(|i| (rgb[i] - c[i]).abs()).fold(0.0f32, f32::max);
            let w = if d <= inner {
                1.0
            } else if d >= tolerance {
                0.0
            } else {
                let t = (tolerance - d) / (tolerance - inner);
                t * t * (3.0 - 2.0 * t)
            };
            best = best.max(w);
        }
        best
    }
}

/// The triangles a [`Selection`] covers, by part index, for per-texel tests. Parts are matched by name.
#[derive(Debug, Clone, Default)]
pub struct PickLookup {
    triangles: HashSet<(usize, usize)>,
}

impl PickLookup {
    /// The picks of `selection`, resolving each part name with `part_index`.
    pub fn new(selection: &Selection, part_index: impl Fn(&str) -> Option<usize>) -> PickLookup {
        let mut triangles = HashSet::new();
        for pick in &selection.picks {
            let Some(part) = part_index(&pick.part) else { continue };
            triangles.extend(pick.triangles.iter().map(|t| (part, *t as usize)));
        }
        PickLookup { triangles }
    }

    pub fn covers(&self, part: usize, triangle: usize) -> bool {
        self.triangles.contains(&(part, triangle))
    }

    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::BakedKind;
    use crate::paint::effect::{Pick, PickMode};

    #[test]
    fn levels_map_ranges_gamma_and_inversion() {
        let id = Levels::default();
        assert!(id.is_identity());
        assert_eq!(id.apply([0.25, 0.5, 1.0, 0.5]), [0.25, 0.5, 1.0, 0.5]);
        let stretch = Levels { input_min: [0.2; 3], input_max: [0.6; 3], ..Levels::default() };
        assert!((stretch.component(0.4, 0) - 0.5).abs() < 1e-6);
        assert_eq!(stretch.component(0.9, 1), 1.0);
        let invert = Levels { output_min: [1.0; 3], output_max: [0.0; 3], ..Levels::default() };
        assert!((invert.apply_grey(0.25) - 0.75).abs() < 1e-6);
        let bright = Levels { gamma: [2.0; 3], ..Levels::default() };
        assert!((bright.component(0.25, 2) - 0.5).abs() < 1e-6);
        let unclamped = Levels { output_max: [2.0; 3], clamp: false, ..Levels::default() };
        assert_eq!(unclamped.component(1.0, 0), 2.0);
        assert_eq!(Levels { clamp: true, ..unclamped }.component(1.0, 0), 1.0);
        let flat = Levels { input_min: [0.5; 3], input_max: [0.5; 3], ..Levels::default() };
        assert_eq!((flat.component(0.4, 0), flat.component(0.6, 0)), (0.0, 1.0));
    }

    #[test]
    fn colour_selection_tolerance_and_hardness() {
        let mut s = ColorSelection { map: BakedKind::Id, colors: vec![[1.0, 0.0, 0.0]], tolerance: 0.1, hardness: 1.0 };
        assert_eq!(s.weight([0.95, 0.05, 0.0]), 1.0);
        assert_eq!(s.weight([0.8, 0.0, 0.0]), 0.0);
        s.hardness = 0.0;
        let w = s.weight([0.95, 0.0, 0.0]);
        assert!(w > 0.0 && w < 1.0, "{w}");
        s.colors.push([0.0, 0.0, 1.0]);
        assert_eq!(s.weight([0.0, 0.0, 1.0]), 1.0);
    }

    #[test]
    fn picks_resolve_parts_by_name() {
        let sel = Selection {
            mode: PickMode::Triangle,
            picks: vec![Pick { part: "b".into(), triangles: vec![3] }, Pick { part: "gone".into(), triangles: vec![1] }],
        };
        let names = ["a", "b"];
        let lookup = PickLookup::new(&sel, |n| names.iter().position(|x| *x == n));
        assert!(lookup.covers(1, 3));
        assert!(!lookup.covers(0, 3));
        assert!(!lookup.is_empty());
    }
}
