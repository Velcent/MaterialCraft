//! Polygon fill picking (PT-45, PT-46, PT-49): which triangles a click, a dragged rectangle or a lasso picks, grown
//! to what the pick mode covers, and how a pick changes a polygon fill's
//! [`Selection`](crate::paint::effect::Selection).
//!
//! - [`PickRegion`]: a point, rectangle or lasso, in screen units (3D view) or UV units (2D view).
//! - [`ScreenPick`]: picks through a [`ViewProjector`](crate::projection::ViewProjector) (the 3D view): a point
//!   picks the nearest triangle under it; a rectangle or lasso every triangle it touches on screen, optionally only
//!   those whose centre the camera sees.
//! - [`uv_pick`]: the same in UV space (the 2D view's hook; nothing is hidden there).
//! - [`MeshTopologies`]: grows picked triangles by [`PickMode`](crate::paint::effect::PickMode): to their source
//!   face ([`crate::mesh::FaceGroups`]), their connected piece of mesh or their UV island.
//! - [`apply_pick`]: adds the picked triangles to a selection (fill) or takes them out (un-pick), by part name, so
//!   the result is data the compositor re-rasterises at any resolution.
//!
//! Only the parts of one material slot (the texture set's) take part: a pick never reaches into another set.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod grow;
mod region;
mod screen;
mod selection;
#[cfg(test)]
mod tests;
mod uv;

pub use grow::MeshTopologies;
pub use region::PickRegion;
pub use screen::ScreenPick;
pub use selection::apply_pick;
pub use uv::uv_pick;

use std::collections::{BTreeMap, BTreeSet};

/// Picked triangles by mesh part index.
pub type Picked = BTreeMap<usize, BTreeSet<u32>>;
