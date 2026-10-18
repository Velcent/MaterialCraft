//! The pen pressure curve's presets (PT-07). The curve itself is a list of `[input, output]` points in the user
//! settings (`[pen] pressure_curve` in `config.toml`, hand-editable); the presets are named point lists the
//! settings panel offers. The stylus evaluates the points as the brush crate's monotone [`ResponseCurve`].
//!
//! [`ResponseCurve`]: materialcraft_brush::ResponseCurve

/// A named pressure curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressurePreset {
    /// Output follows the pen.
    Linear,
    /// A light touch already paints strongly.
    Soft,
    /// Strong output needs a firm press.
    Firm,
}

impl PressurePreset {
    pub const ALL: [PressurePreset; 3] = [PressurePreset::Linear, PressurePreset::Soft, PressurePreset::Firm];

    pub fn label(self) -> &'static str {
        match self {
            PressurePreset::Linear => "Linear",
            PressurePreset::Soft => "Soft",
            PressurePreset::Firm => "Firm",
        }
    }

    /// The curve's control points.
    pub fn points(self) -> Vec<[f32; 2]> {
        match self {
            PressurePreset::Linear => vec![[0.0, 0.0], [1.0, 1.0]],
            PressurePreset::Soft => vec![[0.0, 0.0], [0.25, 0.5], [1.0, 1.0]],
            PressurePreset::Firm => vec![[0.0, 0.0], [0.5, 0.25], [1.0, 1.0]],
        }
    }

    /// The preset `points` are, if any (`None` for a hand-edited curve).
    pub fn of(points: &[[f32; 2]]) -> Option<PressurePreset> {
        PressurePreset::ALL.into_iter().find(|p| p.points() == points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_brush::ResponseCurve;

    #[test]
    fn presets_are_recognised_and_shape_pressure_as_named() {
        for p in PressurePreset::ALL {
            assert_eq!(PressurePreset::of(&p.points()), Some(p));
        }
        assert_eq!(PressurePreset::of(&[[0.0, 0.1], [1.0, 1.0]]), None);
        let at = |p: PressurePreset, x: f32| ResponseCurve::new(&p.points()).eval(x);
        assert_eq!(at(PressurePreset::Linear, 0.4), 0.4);
        assert!(at(PressurePreset::Soft, 0.25) > 0.45 && at(PressurePreset::Firm, 0.5) < 0.3);
        for p in PressurePreset::ALL {
            assert!((at(p, 0.0), at(p, 1.0)) == (0.0, 1.0), "{p:?} keeps the ends");
        }
    }
}
