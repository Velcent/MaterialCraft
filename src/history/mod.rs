//! Undo history: one linear, project-wide timeline of named steps per open document (session only, nothing
//! is saved).
//!
//! A step holds the inverses of the edits it made ([`Command`]s, for paint documents the
//! [`crate::paint::edit::Edit`]s): structural changes as small commands, pixel changes as the old shared tiles
//! of the area painted. Undo applies the inverses and keeps what they return for redo, so a step costs the
//! changed data only, never a copy of the document.
//!
//! - **Steps** have a label and a time for the timeline ([`History::steps`]); [`History::jump_to`] returns the
//!   whole document to any of them. A new edit after stepping back drops the undone steps.
//! - **Coalescing**: edits recorded with the same [`CoalesceKey`] while the step is open join it, so one
//!   slider drag or one brush stroke is one step; [`Command::absorb`] keeps such a step small (the first old
//!   value, the first old tile of each tile).
//! - **Budget**: the bytes of the shared allocations only the history keeps alive (old tiles, removed layers),
//!   each counted once however many steps share it, stay under [`History::budget`]; the oldest steps are
//!   dropped first, the newest is always kept. The design of the budget follows PhotoCraft's `ops` crate.
//! - Shader settings keep their own history of their own command type with the same mechanism; it never
//!   appears on the project timeline.
//!
//! ```
//! use materialcraft::history::{CoalesceKey, PaintHistory};
//! use materialcraft::paint::edit::{Edit, LayerProperty, LayerRef, LayerSlot};
//! use materialcraft::paint::{Document, Layer, LayerKind, PaintContent, TextureSet};
//!
//! let mut doc = Document::default();
//! doc.texture_sets.push(TextureSet::new("s", "Set", (256, 256)));
//! let mut history = PaintHistory::default();
//! let layer = Layer::new("l", "Layer", LayerKind::Paint(PaintContent::default()));
//! history.apply(&mut doc, "Add layer", None, Edit::InsertLayer { set: "s".into(), at: LayerSlot::top(0), layer: Box::new(layer) }).unwrap();
//! // A slider drag: many edits, one step.
//! let drag = Some(CoalesceKey(1));
//! for v in [0.9, 0.7, 0.5] {
//!     history.apply(&mut doc, "Opacity", drag, Edit::SetLayer { layer: LayerRef::new("s", "l"), property: LayerProperty::Opacity(v) }).unwrap();
//! }
//! history.close();
//! assert_eq!(history.len(), 2);
//! history.undo(&mut doc).unwrap();
//! assert_eq!(doc.texture_sets[0].layer("l").unwrap().opacity, 1.0);
//! history.jump_to(&mut doc, 0).unwrap();
//! assert!(doc.texture_sets[0].layers.is_empty());
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod budget;
mod command;
mod paint;
mod shading;
mod stack;
mod step;

#[cfg(test)]
mod tests;

pub use command::Command;
pub use stack::{DEFAULT_BUDGET, History};
pub use step::{CoalesceKey, StepInfo};

/// The project history of a paint document.
pub type PaintHistory = History<crate::paint::edit::Edit>;

/// The shader settings' own history (VS-06): never on the project timeline.
pub type ShaderHistory = History<crate::shading::ShaderEdit>;
