//! Layer import (M5): a Painter texture set's layer stack ([`crate::model::PainterTextureSet`]) as our
//! layers ([`materialcraft::paint::Layer`]), and how exactly they reproduce Painter.
//!
//! - [`blend_table`]: the one mapping of Painter's blend values to ours, and Painter's defaults.
//! - [`blending`]: the blend mode and opacity a channel gets from a stored list.
//! - [`channels`]: declared channels on both sides; Painter's linear values in our encoding.
//! - [`content`] and [`layer_content`]: a layer's effect stack → per-channel values → our layer kind.
//! - [`mask`]: masks made of uniform/bitmap fills → mask images; [`images`]: bitmap resources.
//! - [`stack_builder`]: the tree walk; [`verdict`] and [`computability`]: what is exact, per channel/layer.
//! - [`cache_score`] and [`cache_agreement`]: a fully computable stack measured against Painter's cache.
//! - [`strategy`]: the per-set decision between the real stack and Painter's flattened result.
//! - [`report`]: the import report lines.

pub mod blend_table;
pub mod blending;
pub mod cache_agreement;
pub mod cache_score;
pub mod channels;
pub mod computability;
pub mod content;
pub mod images;
pub mod layer_content;
pub mod mask;
pub mod report;
pub mod stack_builder;
pub mod strategy;
#[cfg(test)]
mod tests;
pub mod verdict;

pub use blend_table::{BlendTable, Confidence};
pub use channels::SetChannels;
pub use computability::Computability;
pub use images::{ImageSource, NoImages, ProjectImages};
pub use stack_builder::ImportedStack;
pub use strategy::{PAINTER_RESULT_NAME, REFERENCE_NAME, StackParts, StackStrategy};
pub use verdict::StackVerdict;

use crate::model::PainterTextureSet;
use stack_builder::StackBuilder;

/// Imports layer stacks with one blend table and one image source.
pub struct LayerImport<'a> {
    pub table: &'a BlendTable,
    pub images: &'a dyn ImageSource,
}

impl LayerImport<'_> {
    /// The set's layers for the declared `channels` at `resolution`.
    pub fn import(&self, set: &PainterTextureSet, channels: &SetChannels, resolution: (u32, u32)) -> ImportedStack {
        StackBuilder {
            channels,
            table: self.table,
            images: self.images,
            resolution,
        }
        .build(&set.layers)
    }
}
