//! [`StackStrategy`]: the one decision per texture set about what the imported document shows.
//!
//! - **Real stack**: every channel of the imported layers reproduces Painter exactly (only visible,
//!   contributing layers count). The layers are the result; the flattened cache layer stays as a hidden
//!   reference at the top ("Painter result (reference)").
//! - **Painter result on top**: something is not computable (procedurals, strokes, a guessed blend mode …).
//!   The layers are imported as they are, and the flattened cache layer ("Painter result") is shown on top,
//!   with Painter's defaults right under it wherever the cache does not cover every texel of every channel,
//!   so the visible result is exactly the flattened import's.
//! - **Cache disagrees**: every layer is computable, but the composite differs from Painter's cache beyond
//!   rounding ([`CacheAgreement`], e.g. a stale cache). Arranged like "Painter result on top", so an import
//!   never looks worse than the flattened one.
//! - **Replacements**: every channel is exact apart from Substance procedurals the user chose to replace with
//!   MaterialCraft's generators. Arranged like the real stack (the layers are the result, editable; Painter's
//!   flattened result is the hidden reference), but not compared with the cache: replacements are measured
//!   approximations.
//! - **Best effort**: not computable and no texture cache to fall back on; the layers are all there is.
//!
//! Layers Painter shows whose mask is not computable would cover the whole set without it: they are hidden,
//! unless "Painter result" is on top anyway (then Painter's visibility is kept so the structure mirrors it).
//!
//! "Painter defaults" (Painter's values for uncovered texels) sits at the bottom unless the layers already
//! cover every channel with an unmasked, full-strength uniform fill.

use super::cache_agreement::CacheAgreement;
use super::channels::SetChannels;
use super::verdict::StackVerdict;
use materialcraft::paint::{BlendMode, Layer, LayerKind};

pub const PAINTER_RESULT_NAME: &str = "Painter result";
pub const REFERENCE_NAME: &str = "Painter result (reference)";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackStrategy {
    RealStack,
    Replacements,
    PainterResultOnTop,
    CacheDisagrees,
    BestEffort,
}

impl StackStrategy {
    /// `agreement` is the measurement of a fully computable stack against the cache (empty when not measured).
    pub fn choose(verdict: &StackVerdict, has_painter_result: bool, agreement: &CacheAgreement) -> StackStrategy {
        if !verdict.all_exact() && verdict.all_exact_or_replaced() {
            return StackStrategy::Replacements;
        }
        match (verdict.all_exact(), has_painter_result) {
            (true, true) if !agreement.agrees() => StackStrategy::CacheDisagrees,
            (true, _) => StackStrategy::RealStack,
            (false, true) => StackStrategy::PainterResultOnTop,
            (false, false) => StackStrategy::BestEffort,
        }
    }

    /// True when the flattened cache layer is shown on top of the imported layers.
    pub fn shows_painter_result(self) -> bool {
        matches!(self, StackStrategy::PainterResultOnTop | StackStrategy::CacheDisagrees)
    }

    pub fn describe(self) -> &'static str {
        match self {
            StackStrategy::RealStack => {
                "layers reproduce Painter exactly and are the result; Painter's flattened result kept hidden as a reference"
            }
            StackStrategy::Replacements => {
                "layers reproduce Painter with MaterialCraft's replacements for Substance procedurals and are the result; Painter's flattened result kept hidden as a reference"
            }
            StackStrategy::PainterResultOnTop => {
                "not every layer is computable: Painter's flattened result is shown on top of the imported layers"
            }
            StackStrategy::CacheDisagrees => {
                "every layer is computable, but Painter's cache disagrees with the stored layers: Painter's flattened result is shown on top of the imported layers"
            }
            StackStrategy::BestEffort => {
                "not every layer is computable and there is no texture cache: the imported layers show what can be computed"
            }
        }
    }
}

/// What a set's final stack is made of.
pub struct StackParts {
    /// The imported layers, bottom → top.
    pub imported: Vec<Layer>,
    /// The flattened cache layer, if the set has a texture cache.
    pub painter_result: Option<Layer>,
    /// Painter's channel defaults as a uniform fill.
    pub defaults: Layer,
    /// True when the cache covers every texel of every declared channel.
    pub result_complete: bool,
    /// Ids of layers to hide when Painter's result is not on top (their masks are not computable).
    pub hide_unless_covered: Vec<String>,
}

impl StackStrategy {
    /// The final layers, bottom → top.
    pub fn assemble(self, parts: StackParts, channels: &SetChannels) -> Vec<Layer> {
        let StackParts {
            imported,
            painter_result,
            defaults,
            result_complete,
            hide_unless_covered,
        } = parts;
        let mut imported = imported;
        if !self.shows_painter_result() {
            hide(&mut imported, &hide_unless_covered);
        }
        let base = provides_base(&imported, channels);
        let mut out = Vec::new();
        match self {
            StackStrategy::RealStack | StackStrategy::Replacements | StackStrategy::BestEffort => {
                if !base {
                    out.push(defaults);
                }
                out.extend(imported);
                if let Some(mut result) =
                    painter_result.filter(|_| matches!(self, StackStrategy::RealStack | StackStrategy::Replacements))
                {
                    result.name = REFERENCE_NAME.into();
                    result.visible = false;
                    out.push(result);
                }
            }
            StackStrategy::PainterResultOnTop | StackStrategy::CacheDisagrees => {
                out.extend(imported);
                if let Some(mut result) = painter_result {
                    if !result_complete {
                        out.push(defaults);
                    }
                    result.name = PAINTER_RESULT_NAME.into();
                    out.push(result);
                }
            }
        }
        out
    }
}

/// Hides the layers with these ids, at any depth.
fn hide(layers: &mut [Layer], ids: &[String]) {
    for l in layers {
        if ids.contains(&l.id) {
            l.visible = false;
        }
        if let LayerKind::Group(children) = &mut l.kind {
            hide(children, ids);
        }
    }
}

/// True if every declared channel is fully covered by a visible, unmasked top-level uniform fill at
/// normal or replace blending and full opacity.
pub fn provides_base(layers: &[Layer], channels: &SetChannels) -> bool {
    channels.all().all(|channel| {
        layers.iter().any(|l| {
            let full = l
                .blend_for(channel)
                .is_some_and(|b| matches!(b.mode, BlendMode::Normal | BlendMode::Replace) && b.opacity >= 1.0);
            let value = matches!(&l.kind, LayerKind::Fill(f) if f.uniform_value(channel).is_some());
            let unmasked = l.mask.as_ref().is_none_or(|m| m.is_identity());
            l.visible && unmasked && l.opacity >= 1.0 && full && value
        })
    })
}
