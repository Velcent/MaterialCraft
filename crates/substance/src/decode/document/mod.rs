//! `/paint/document.bin`: the layer stack in Painter's own object serialisation. The same
//! format is used by `/paint/default_material.bin` and `/editor/layersstackstate.bin`.
//! The walker streams the object tree to a [`Visitor`]; [`ClassCensus`] counts classes and
//! [`DocumentMaterials`] reads the texture-set records the importer needs.

mod census;
mod context;
mod header;
mod materials;
#[cfg(test)]
mod materials_tests;
mod tag;
#[cfg(test)]
mod test_writer;
#[cfg(test)]
mod tests;
mod value;
mod walker;

pub use census::ClassCensus;
pub use context::{Context, Frame};
pub use header::DocumentHeader;
pub use materials::{BitmapRef, ChannelRecord, DocumentMaterials, MaterialRecord};
pub use tag::{Payload, ValueTag};
pub use value::Scalar;
pub use walker::{walk, Visitor};

const RECORD: &str = "document";
