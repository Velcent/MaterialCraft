//! Asset libraries: the engine of the asset tray, std-only and headless (roadmap step 9,
//! requirements AS-01 to AS-23 in MaterialCraft-docs).
//!
//! The editor reads the library roots and saved searches from its settings and passes them in as
//! plain values. A [`Library`] scans each folder-backed root into a [`Catalogue`], keeps it current
//! with a polling [`Watcher`], and answers [`Search`]es. Problems found while scanning (unreadable
//! folders, malformed sidecars) go to a [`diag::Sink`](crate::diag::Sink) under `Area::Assets`; they
//! never stop a scan.
//!
//! ```text
//! roots (from settings) --> Listing::read --> Catalogue::load ----> Catalogue::find(Search) --> tray
//!                              Watcher::poll --> FileChange --> Catalogue::update
//! ```
//!
//! Only MaterialCraft's own formats are assets (`.mca` graphs, PNG, OpenEXR); each may have a JSON
//! sidecar (`<file>.mcmeta`) with name, usage, tags, thumbnail and provenance, and a conventional
//! thumbnail (`<file>.thumb.png`). `.sbsar` files are listed as importable only (ADR 0008).
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod asset;
mod build;
mod catalogue;
mod format;
mod layout;
mod library;
mod listing;
mod provenance;
mod query;
mod root;
mod search;
mod sidecar;
mod usage;
mod watch;

pub use asset::{Asset, Thumbnail};
pub use build::MAX_SIDECAR_BYTES;
pub use catalogue::{AssetChange, Catalogue, Hit};
pub use format::{FileRole, Format, SIDECAR_SUFFIX, THUMBNAIL_SUFFIX};
pub use layout::create_standard_layout;
pub use library::{Library, Refresh};
pub use listing::{ChangeKind, FileChange, Listing, Stamp};
pub use provenance::{CC0, Provenance};
pub use query::{Field, Mode, Query, Term};
pub use root::{LibraryRoot, RootId, RootKind};
pub use search::{Location, SavedSearch, Search};
pub use sidecar::{SIDECAR_FORMAT, SIDECAR_VERSION, Sidecar, SidecarError};
pub use usage::Usage;
pub use watch::{RootChanges, Watcher};
