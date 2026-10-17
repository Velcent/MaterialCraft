//! Per texture set bake configuration (roadmap step 28, requirements BK2-01…BK2-05, BK-02…BK-07).
//!
//! Every [`crate::paint::TextureSet`] carries a [`SetBakeConfig`] (`TextureSet::bake`): the common settings
//! ([`CommonBake`]: size, ray distances, sampling, dilation, high-poly source) and one [`MapEntry`] per bakeable map
//! with its [`MapFlags`] (include in the bake, show in the viewport, auto-rebake) and its baker's [`MapSettings`].
//! The document carries the [`BakeLinks`] (`Document::bake_links`): which parts are linked across sets.
//!
//! - [`config`]: the types and their conversion to and from the baker's [`crate::bake::BakeSettings`];
//! - [`links`]: link flags; [`ops`]: link, copy and check commands as undoable document operations;
//! - [`key`]: [`SettingKey`], a setting as a log entry's target; [`kinds`]: map kinds and storing results.
//!
//! The settings types are the baker's own, so a configuration bakes exactly what [`crate::bake::bake_set`] is given.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod config;
pub mod key;
pub mod kinds;
pub mod links;
pub mod ops;

#[cfg(test)]
mod tests;

pub use config::{CommonBake, HighPlacement, HighPoly, MapEntries, MapEntry, MapFlags, MapSettings, SetBakeConfig};
pub use key::SettingKey;
pub use kinds::{baked_kind, map_kind, to_baked};
pub use links::{BakeLinks, BakePart};
