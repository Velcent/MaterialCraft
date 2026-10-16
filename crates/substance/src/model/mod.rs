//! Stage 3 of the import pipeline: Painter's own concepts as typed Rust, built from the `document.bin`
//! object tree. A faithful mirror (Painter's names, numbers and ordering); mapping to MaterialCraft's
//! paint model is `translate`'s job. Field meanings and evidence: `interop/painter-document-catalogue.md`.
//!
//! Entry point: [`PainterDocument::from_bytes`] → ([`PainterDocument`], [`ModelReport`]).
//! - [`raw`]: the generic tree ([`RawTree`]) the builder reads.
//! - [`PainterDocument`] → [`PainterTextureSet`]s → [`PainterLayer`]s (bottom → top), each with
//!   [`ChannelBlending`] entries, an optional [`PainterMask`] and a content stack of [`PainterEffect`]s.
//! - Effects ([`FillEffect`], [`PaintEffect`], filters, levels, …) take values from [`PainterSource`]s.
//! - Unknown classes become [`RawEntry`]s; [`ModelReport`] counts recognised, raw and unmirrored objects.

mod blending;
mod builder;
mod channel;
mod document;
mod effect;
mod fill;
mod layer;
mod mask;
mod paint;
mod procedural;
pub mod raw;
mod raw_entry;
mod report;
mod source;
mod summary;
#[cfg(test)]
mod tests;
mod texture_set;

pub use blending::{blending_for, ChannelBlending, PainterBlendMode};
pub use channel::{ChannelMask, PainterChannel};
pub use document::PainterDocument;
pub use effect::{
    ColorSelectionEffect, EffectGroup, EffectHeader, FilterEffect, Levels, LevelsEffect, PainterEffect,
    PolygonFillEffect,
};
pub use fill::FillEffect;
pub use layer::{LayerKind, PainterLayer};
pub use mask::{MaskInitial, PainterMask};
pub use paint::{PaintEffect, PaintTool};
pub use procedural::{Procedural, ProceduralInput, ProceduralOutput, Tweak, TweakValue};
pub use raw::RawTree;
pub use raw_entry::RawEntry;
pub use report::{ClassTally, ModelReport};
pub use source::{BitmapSource, PainterSource, ProceduralSource, ReferenceSource, UniformSource};
pub use summary::{ContentKind, StackSummary};
pub use texture_set::{ChannelDecl, PainterTextureSet};
