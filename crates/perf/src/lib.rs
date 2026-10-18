//! Paint mode performance scenarios (roadmap step 52): what a benchmark does, described as data, and what it
//! measured, kept as a history.
//!
//! - [`scenario`]: a [`Scenario`] (a source document or mesh, an action, repetitions) and its JSON form;
//!   [`builtin`] is the shipped table, [`catalog`] adds the user's scenario files to it.
//! - [`report`]: a [`RunReport`], the flat, named numbers one run of a scenario measured.
//! - [`machine`], [`history`], [`compare`]: where a run happened, the JSON lines file it is appended to, and the
//!   change against the previous run of the same scenario on the same machine.
//! - [`cli`]: the `mcperf` command. The runs themselves happen in the editor binary (`materialcraft-gui
//!   --perf-run`), which drives the Paint screen's real components; [`editor`] starts it.
//!
//! The runner lives in the editor (`crates/editor/src/gui_paint/perf`); this crate knows nothing of the editor.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod builtin;
pub mod catalog;
pub mod cli;
pub mod compare;
pub mod editor;
pub mod history;
pub mod machine;
pub mod report;
pub mod scenario;

pub use report::{RunReport, Stat, Status};
pub use scenario::{Action, EditKind, LayerChoice, PathShape, Scenario, Source, StrokeSpec, StrokeTarget, StrokeTool, Symmetry};
