//! `/paint/document.bin`: the layer stack in Painter's own object serialisation. The same
//! format is used by `/paint/default_material.bin` and `/editor/layersstackstate.bin`.
//! M1 does not decode values; it walks the object tree and counts which `Data*` classes occur.

mod census;
mod header;
mod tag;
#[cfg(test)]
mod test_writer;
#[cfg(test)]
mod tests;
mod walker;

pub use census::ClassCensus;
pub use header::DocumentHeader;
pub use tag::{Payload, ValueTag};
pub use walker::{walk, Visitor};

const RECORD: &str = "document";
