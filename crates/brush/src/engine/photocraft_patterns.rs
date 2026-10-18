//! PhotoCraft's procedural patterns (`photocraft_paint::procedural::pattern`): seamless, seeded grey tiles (value
//! noise, paper tooth, woven canvas, halftone dots) written for PhotoCraft from scratch. The starter library ships them
//! as textures. Besides the other `photocraft*` files of this module, this is the only file naming `photocraft_paint`.

use photocraft_paint::brush::PatternStyle;
use photocraft_paint::procedural as pc;

/// The generator a texture made from a pattern records in its provenance.
pub const PATTERN_GENERATOR: &str = "photocraft-paint.pattern";

/// Smallest and largest side PhotoCraft makes a pattern at.
pub const PATTERN_PX: std::ops::RangeInclusive<u32> = 8..=1024;

/// The periodic patterns (canvas and dots) repeat every this many pixels: a side that is a multiple of it tiles.
pub const PATTERN_PERIOD: u32 = 8;

/// One of PhotoCraft's pattern styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternKind {
    /// Smooth fractal value noise.
    Noise,
    /// Paper tooth: coarse noise with per-pixel grain.
    Paper,
    /// Woven canvas, over and under threads.
    Canvas,
    /// Round halftone dots.
    Dots,
}

impl PatternKind {
    pub const ALL: [PatternKind; 4] = [PatternKind::Noise, PatternKind::Paper, PatternKind::Canvas, PatternKind::Dots];

    /// The style's id, as provenance records it.
    pub fn id(self) -> &'static str {
        match self {
            PatternKind::Noise => "noise",
            PatternKind::Paper => "paper",
            PatternKind::Canvas => "canvas",
            PatternKind::Dots => "dots",
        }
    }

    fn style(self) -> PatternStyle {
        match self {
            PatternKind::Noise => PatternStyle::Noise,
            PatternKind::Paper => PatternStyle::Paper,
            PatternKind::Canvas => PatternStyle::Canvas,
            PatternKind::Dots => PatternStyle::Dots,
        }
    }
}

/// The pattern `kind` with `seed`, `side` × `side`, row by row, 0..1; `None` when `side` is outside [`PATTERN_PX`].
/// Seamless when `side` is a multiple of [`PATTERN_PERIOD`].
pub fn pattern(kind: PatternKind, side: u32, seed: u32) -> Option<Vec<f32>> {
    if !PATTERN_PX.contains(&side) {
        return None;
    }
    let image = pc::pattern(kind.style(), side, seed);
    let n = side as usize;
    (image.width == n && image.height == n && image.data.len() == n * n).then(|| image.data.iter().map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_renders_repeatably_and_tiles() {
        for kind in PatternKind::ALL {
            let n = 64usize;
            let a = pattern(kind, n as u32, 3).unwrap();
            assert_eq!(a.len(), n * n);
            assert_eq!(Some(a.clone()), pattern(kind, n as u32, 3), "{kind:?}");
            assert!(a.iter().any(|v| *v > 0.1), "{kind:?} is empty");
            // seamless: the jump across the wrap is no bigger than the largest jump inside
            let at = |x: usize, y: usize| a[(y % n) * n + (x % n)];
            let inside = (0..n).flat_map(|y| (0..n - 1).map(move |x| (x, y))).map(|(x, y)| (at(x, y) - at(x + 1, y)).abs()).fold(0.0, f32::max);
            let wrap = (0..n).map(|y| (at(n - 1, y) - at(0, y)).abs()).fold(0.0, f32::max);
            assert!(wrap <= inside + 1e-6, "{kind:?}: wrap {wrap} inside {inside}");
        }
        assert!(pattern(PatternKind::Noise, 4, 1).is_none());
        assert_ne!(pattern(PatternKind::Paper, 32, 1), pattern(PatternKind::Paper, 32, 2));
    }
}
