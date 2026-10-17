//! MaterialCraft's brush strokes (roadmap step 20): input points → dabs → texture-space stamps, as one history step
//! and one stroke record per stroke.
//!
//! - [`engine`]: [`DabEngine`] turns input points into dabs. It is PhotoCraft's brush engine (`photocraft-paint`
//!   at a pinned revision) behind one adapter file, `engine::photocraft`, the only code naming it (ADR 0007).
//!   Model-sized brushes get a per-point dab scale there (ADR 0006 Q-PT-12).
//! - [`session`]: [`StrokeSession`] drives one stroke through the core's stamp stage and records its samples.
//! - [`target`]: [`PaintTarget`], what a tool paints (a paint layer's channels or a mask, ADR 0008) and with which
//!   values.
//! - [`document`]: [`DocumentStroke`], a stroke into a document with the inverses for one undo step.
//! - [`replay`]: painting a recorded stroke again, bit for bit.
//! - [`stroke`]: input transforms in front of a session: [`LazyMouse`], straight lines ([`LineAnchor`]).
//! - [`symmetry`]: symmetric copies in a stroke record's form, built the same way live and on replay.
//!
//! Rule: the std-only core never depends on this crate; it sees only plain dabs and coverage grids.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod document;
pub mod engine;
pub mod replay;
pub mod session;
pub mod stroke;
pub mod symmetry;
pub mod target;

pub use document::{DocumentStroke, Ended, Prepared};
pub use engine::{DabEngine, InputPoint};
pub use replay::{ReplayScene, recorded_brush, recorded_values, replay};
pub use session::{Finished, SessionScene, StrokeSession, StrokeSetup};
pub use stroke::{LazyMouse, LineAnchor};
pub use target::{PaintTarget, PaintValues, Resolved, TargetPart};

use materialcraft::paint::edit::EditError;
use materialcraft::stamp::StampError;
use std::fmt;

/// Why a stroke could not be painted.
#[derive(Debug, Clone, PartialEq)]
pub enum BrushError {
    /// The target can't be painted (the text says why).
    NotPaintable(&'static str),
    /// A 3D-view stroke without a view.
    NoView,
    /// A recorded stroke can't be replayed (the text says why).
    NotReplayable(&'static str),
    Stamp(StampError),
    Edit(EditError),
}

impl fmt::Display for BrushError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BrushError::NotPaintable(why) | BrushError::NotReplayable(why) => f.write_str(why),
            BrushError::NoView => f.write_str("the 3D view is not ready"),
            BrushError::Stamp(e) => write!(f, "{e}"),
            BrushError::Edit(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BrushError {}

impl From<StampError> for BrushError {
    fn from(e: StampError) -> BrushError {
        BrushError::Stamp(e)
    }
}
