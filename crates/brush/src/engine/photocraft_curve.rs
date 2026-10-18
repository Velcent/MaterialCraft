//! PhotoCraft's response curve (`photocraft_paint::ResponseCurve`, pinned; ADR 0007), the one other file besides
//! the dab engine's adapter and the tips that names `photocraft_paint`. The editor shapes pen pressure with it
//! before any brush sees the pressure (PT-07: "editable response curves").

/// A monotone curve (Fritsch–Carlson cubic) through `[input, output]` control points in 0 … 1: a stronger input
/// never gives less output. Points are sanitised (non-finite ones dropped, clamped, sorted, at most
/// [`ResponseCurve::MAX_POINTS`], outputs made non-decreasing); fewer than two usable points is linear.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ResponseCurve(photocraft_paint::ResponseCurve);

impl ResponseCurve {
    /// Most control points a curve keeps.
    pub const MAX_POINTS: usize = photocraft_paint::ResponseCurve::MAX_POINTS;

    /// The identity: input passes through unchanged.
    pub const LINEAR: ResponseCurve = ResponseCurve(photocraft_paint::ResponseCurve::LINEAR);

    pub fn new(points: &[[f32; 2]]) -> ResponseCurve {
        ResponseCurve(photocraft_paint::ResponseCurve::new(points))
    }

    /// The output for `x` (clamped to 0 … 1; NaN reads as 0).
    pub fn eval(&self, x: f32) -> f32 {
        self.0.eval(x)
    }

    /// The control points as sanitised.
    pub fn points(&self) -> Vec<[f32; 2]> {
        self.0.points()
    }

    /// True for the identity.
    pub fn is_linear(&self) -> bool {
        self.0.is_linear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_its_points_and_never_falls() {
        let c = ResponseCurve::new(&[[0.0, 0.0], [0.3, 0.6], [1.0, 1.0]]);
        assert!((c.eval(0.3) - 0.6).abs() < 1e-5 && c.eval(0.0) == 0.0 && c.eval(1.0) == 1.0);
        let ys: Vec<f32> = (0..=100).map(|i| c.eval(i as f32 / 100.0)).collect();
        assert!(ys.windows(2).all(|w| w[1] >= w[0]), "{ys:?}");
        assert!(c.eval(f32::NAN) == 0.0 && c.eval(7.0) == 1.0);
    }

    #[test]
    fn bad_points_fall_back_to_linear() {
        assert!(ResponseCurve::new(&[]).is_linear());
        assert!(ResponseCurve::new(&[[f32::NAN, 0.0], [0.5, 0.5]]).is_linear());
        assert_eq!(ResponseCurve::LINEAR.eval(0.25), 0.25);
        assert_eq!(ResponseCurve::default(), ResponseCurve::LINEAR);
    }
}
