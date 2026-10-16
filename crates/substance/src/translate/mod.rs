//! Stage 4 of the import pipeline: an opened `.spp` → [`materialcraft::paint::Document`] with an import report.
//! The only part of the crate that knows both Painter's names and ours; the mapping tables live in
//! [`channel_table`], [`source_format`] and [`normal_conventions`].
//!
//! Each texture set gets Painter's layer stack ([`layers`], M5) and the flattened channels from the texture
//! cache (M3) as one paint layer, either on top (when not every layer is computable) or as a hidden reference,
//! plus its baked mesh maps; everything not translated is kept under `foreign/substance/`.

mod baked_maps;
pub(crate) mod cache_channels;
pub mod channel_table;
mod error;
mod foreign;
mod importer;
pub mod layers;
pub mod normal_conventions;
mod pixel_convert;
mod report;
mod resources;
mod set_builder;
pub(crate) mod set_layers;
pub mod source_format;
mod texture_sets;

pub use error::{ImportError, PartError};
pub use foreign::FOREIGN_ROOT;
pub use importer::{import_spp, Importer};
pub use layers::{PAINTER_RESULT_NAME, REFERENCE_NAME};
pub use set_layers::{DEFAULTS_LAYER_ID, DEFAULTS_LAYER_NAME, PAINTER_RESULT_ID};
pub use texture_sets::{SetBinding, SetSource};
