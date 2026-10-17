//! Brush presets: MaterialCraft's own description of a brush tool (AS-06), the data the brush crate turns into
//! stamps. No engine type appears here; `crates/brush` converts presets for the engine it uses.
//!
//! - [`BrushPreset`]: size and what it is measured in, tip, hardness, flow, stroke opacity, spacing, angle,
//!   jitter, pressure use, placement mode and the facing limit (PT-04 … PT-11), as JSON in our own format.
//! - [`RecordedBrush`]: a preset plus the painted values and viewport, stored with each stroke we record so the
//!   stroke can be replayed.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod json;
mod preset;
mod recorded;
#[cfg(test)]
mod tests;

pub use json::PRESET_VERSION;
pub use preset::{BrushPreset, Jitter, Placement, PressureUse, SizeSpace, Tip};
pub use recorded::{REFERENCE_PREFIX, RecordedBrush};
