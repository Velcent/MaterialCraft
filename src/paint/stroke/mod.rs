//! Stroke records: the brush strokes behind painted pixels, kept as data next to them (ADR 0007, "store both").
//!
//! A paint layer, a content paint effect and a mask paint effect may carry a [`StrokeData`]: input samples
//! (position, pressure, tilt, time) per [`Stroke`] with the camera as plain matrices, the view kind (3D or UV),
//! the tool, a brush preset reference, the random seed and the symmetry. The pixels stay what is shown; nothing
//! replays a record yet. Importers fill records from the source program's strokes; `.mcpaint` stores each as a
//! compact binary entry ([`crate::paint::format`]). A record that could not be read is kept as its bytes
//! ([`UnreadableStrokes`]) and listed by [`problems`].
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod attach;
mod camera;
mod record;
#[cfg(test)]
mod tests;

pub use attach::{StrokeOwner, StrokeProblem, problems, totals};
pub use record::{Camera, IDENTITY, Mat4, Stroke, StrokeData, StrokeRecord, StrokeSample, StrokeTool, Symmetry, UnreadableStrokes, ViewKind};
