//! [`BlendTable`]: THE mapping from Painter's stored blend mode (`DataBlending.blendingMode`) to ours, with
//! how sure each entry is, plus Painter's per-channel defaults for entries a sparse list leaves out.
//! Evidence per value: `interop/painter-document-catalogue.md`, "Blend modes".
//!
//! Pixel fits (`mcsub verify-layers --all --fit`, whole corpus, with levels, references, colour selection, polygon
//! fill and stacked fills computable, 2026-10-17): only 2, 10 and 27 occur in a channel that can be compared.
//! 2 → normal: 131 channels, mean 0.16 steps (128 reproduced; next best replace 2.89). 10 → add: 10 height
//! channels, 0.00, but every one adds onto zero height, where all modes tie. 27: 42 normal channels, 0.00, all
//! with flat normals, where every mode ties. No other value has pixels in the corpus (fit error unknown); they
//! stay guesses until a Painter-made test file exists (`research/painter-test-recipes.md`).

use crate::model::{PainterBlendMode, PainterChannel};
use materialcraft::paint::BlendMode;

/// How well a mapping is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Confidence {
    /// Named from file evidence (where it is used, Painter's own defaults).
    Identified,
    /// Fitted against Painter's texture cache on the corpus.
    Fitted,
    /// A nearest guess: imported, but reported as approximated.
    Guessed,
}

impl Confidence {
    /// True if a layer using the mode can still be called exact.
    pub fn is_exact(self) -> bool {
        self != Confidence::Guessed
    }
}

/// One mapped mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendMapping {
    pub mode: BlendMode,
    pub confidence: Confidence,
}

/// Painter value → our mode. Values Painter can store are 0–31; anything else maps to `Normal`, guessed.
const PAINTER_BLEND_MODES: &[(u64, BlendMode, Confidence)] = &[
    (0, BlendMode::Disable, Confidence::Identified),
    (1, BlendMode::Replace, Confidence::Identified),
    (2, BlendMode::Normal, Confidence::Identified),
    (3, BlendMode::Normal, Confidence::Guessed),
    (4, BlendMode::Multiply, Confidence::Identified),
    (5, BlendMode::Divide, Confidence::Guessed),
    (6, BlendMode::InverseDivide, Confidence::Guessed),
    (7, BlendMode::Darken, Confidence::Guessed),
    (8, BlendMode::Lighten, Confidence::Guessed),
    (9, BlendMode::Screen, Confidence::Guessed),
    (10, BlendMode::Add, Confidence::Identified),
    (11, BlendMode::Subtract, Confidence::Guessed),
    (12, BlendMode::Difference, Confidence::Guessed),
    (13, BlendMode::Exclusion, Confidence::Guessed),
    (14, BlendMode::SignedAdd, Confidence::Guessed),
    (15, BlendMode::Overlay, Confidence::Guessed),
    (16, BlendMode::Screen, Confidence::Guessed),
    (17, BlendMode::LinearBurn, Confidence::Guessed),
    (18, BlendMode::ColorBurn, Confidence::Guessed),
    (19, BlendMode::ColorDodge, Confidence::Guessed),
    (20, BlendMode::SoftLight, Confidence::Guessed),
    (21, BlendMode::HardLight, Confidence::Guessed),
    (22, BlendMode::VividLight, Confidence::Guessed),
    (23, BlendMode::LinearLight, Confidence::Guessed),
    (24, BlendMode::PinLight, Confidence::Guessed),
    (25, BlendMode::Passthrough, Confidence::Identified),
    (26, BlendMode::NormalMapDetail, Confidence::Identified),
    (27, BlendMode::NormalMapCombine, Confidence::Guessed),
    (28, BlendMode::NormalMapInverseDetail, Confidence::Guessed),
    (29, BlendMode::Subtract, Confidence::Guessed),
    (30, BlendMode::Saturation, Confidence::Guessed),
    (31, BlendMode::Color, Confidence::Guessed),
];

/// The mapping in use. [`BlendTable::painter`] is the importer's; fits try other modes for one value with
/// [`BlendTable::with_override`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlendTable {
    overrides: Vec<(u64, BlendMode)>,
}

impl BlendTable {
    pub fn painter() -> BlendTable {
        BlendTable::default()
    }

    /// The same table with `value` mapped to `mode` (counted as fitted).
    pub fn with_override(mut self, value: u64, mode: BlendMode) -> BlendTable {
        self.overrides.retain(|(v, _)| *v != value);
        self.overrides.push((value, mode));
        self
    }

    pub fn map(&self, mode: PainterBlendMode) -> BlendMapping {
        let value = mode.raw();
        if let Some((_, mode)) = self.overrides.iter().find(|(v, _)| *v == value) {
            return BlendMapping {
                mode: *mode,
                confidence: Confidence::Fitted,
            };
        }
        PAINTER_BLEND_MODES.iter().find(|(v, ..)| *v == value).map_or(
            BlendMapping {
                mode: BlendMode::Normal,
                confidence: Confidence::Guessed,
            },
            |(_, mode, confidence)| BlendMapping {
                mode: *mode,
                confidence: *confidence,
            },
        )
    }
}

/// What holds a blending list; decides the default for a channel the list leaves out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holder {
    /// A layer, folder or content effect.
    Layer,
    /// An effect group (`DataActionGroup`).
    EffectGroup,
    /// A filter-like effect that transforms what is below it (levels): without a stored entry it replaces its
    /// input (normal at full strength) in every channel.
    Filter,
}

/// Painter's default mode for a channel missing from a blending list (format ≥ 61 stores only entries
/// that differ). From the full lists of format ≤ 59: normal except height = linear dodge, normal = 27,
/// ambient occlusion = multiply; effect groups default to passthrough, levels (which store no list) to normal.
pub fn default_mode(channel: PainterChannel, holder: Holder) -> PainterBlendMode {
    match (holder, channel) {
        (Holder::EffectGroup, _) => PainterBlendMode::Passthrough,
        (Holder::Filter, _) => PainterBlendMode::Normal,
        (_, PainterChannel::Height) => PainterBlendMode::LinearDodge,
        (_, PainterChannel::Normal) => PainterBlendMode::Unknown(27),
        (_, PainterChannel::AmbientOcclusion) => PainterBlendMode::Multiply,
        _ => PainterBlendMode::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identified_values_and_guesses() {
        let t = BlendTable::painter();
        let map = |v| t.map(PainterBlendMode::from_raw(v));
        assert_eq!(map(10).mode, BlendMode::Add);
        assert_eq!(map(25).mode, BlendMode::Passthrough);
        assert!(map(4).confidence.is_exact());
        assert!(!map(11).confidence.is_exact());
        assert_eq!(
            map(99),
            BlendMapping {
                mode: BlendMode::Normal,
                confidence: Confidence::Guessed
            }
        );
        for v in 0..32 {
            assert!(PAINTER_BLEND_MODES.iter().any(|(x, ..)| *x == v), "value {v} missing");
        }
    }

    #[test]
    fn overrides_are_fitted() {
        let t = BlendTable::painter()
            .with_override(11, BlendMode::Overlay)
            .with_override(11, BlendMode::Screen);
        let m = t.map(PainterBlendMode::from_raw(11));
        assert_eq!((m.mode, m.confidence), (BlendMode::Screen, Confidence::Fitted));
    }

    #[test]
    fn defaults_follow_the_catalogue() {
        assert_eq!(
            default_mode(PainterChannel::Height, Holder::Layer),
            PainterBlendMode::LinearDodge
        );
        assert_eq!(
            default_mode(PainterChannel::BaseColor, Holder::Layer),
            PainterBlendMode::Normal
        );
        assert_eq!(
            default_mode(PainterChannel::Height, Holder::EffectGroup),
            PainterBlendMode::Passthrough
        );
    }
}
