//! Parity: measures how close MaterialCraft's own implementations get to Substance Painter's results, on the
//! user's own projects, without ever running or decoding Adobe code.
//!
//! The reference (Painter's result) is read from a `.spp` at run time; our output is computed from the same
//! inputs; both are compared inside the UV islands. Nothing read from a project is written into the repo:
//! images go to a cache folder, numbers to the history file.
//!
//! Layers, each using only the ones above it:
//! - [`subject`]: what is compared ([`subject::Subject`]); [`subjects`] lists the implementations.
//! - [`corpus`], [`project`], [`settings`], [`inputs`], [`reference`]: finding projects and loading what a case
//!   needs from them.
//! - [`metrics`], [`image_ops`]: comparing two images.
//! - [`run`]: running cases; [`report`] and [`history`]: writing the results; [`cli`]: the command line.

pub mod cli;
pub mod corpus;
pub mod history;
pub mod image_ops;
pub mod inputs;
pub mod metrics;
pub mod project;
pub mod provenance;
pub mod reference;
pub mod report;
pub mod run;
pub mod settings;
pub mod subject;
pub mod subjects;
