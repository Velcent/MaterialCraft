//! Stage 4 of the import pipeline: an opened `.spp` → [`materialcraft::paint::Document`] with an import report.
//! The only part of the crate that knows both Painter's names and ours; the mapping tables live in
//! [`channel_table`] and [`source_format`].
//!
//! Milestone M3 ("flattened import"): each texture set gets one paint layer holding the flattened channels
//! from the texture cache, plus its baked mesh maps; everything not translated is kept under
//! `foreign/substance/`.

mod baked_maps;
mod cache_channels;
pub mod channel_table;
mod error;
mod foreign;
mod importer;
mod pixel_convert;
mod report;
mod set_builder;
pub mod source_format;
mod texture_sets;

pub use error::{ImportError, PartError};
pub use foreign::FOREIGN_ROOT;
pub use importer::{import_spp, Importer};
pub use set_builder::{DEFAULTS_LAYER_ID, DEFAULTS_LAYER_NAME, IMPORTED_LAYER_ID, IMPORTED_LAYER_NAME};
pub use texture_sets::{SetBinding, SetSource};
