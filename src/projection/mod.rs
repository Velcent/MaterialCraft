//! Projections: how a fill's source (an image, a generator, a material) is laid onto a texture set.
//!
//! A *projection* is data ([`Projection`]: a mode with its settings plus the shared filter, repetition and image
//! placement). [`Projection::prepare`] turns it into a [`Prepared`] projector once per fill and pass; a projector
//! maps a [`SurfacePoint`] (what lies under one texel) to at most three [`Taps`] (image coordinates with weights and
//! footprints) and a coverage. A fill's value at a texel is `coverage × Σ weight × source(tap)`; the source never
//! knows which mode placed it.
//!
//! Modes ([`ProjectionMode`]): texture space (the source laid 1:1 over the UV tile, no settings), UV (an image
//! placement in UV space), and the volume-based modes tri-planar, planar, spherical, cylindrical and warp. Texture
//! space and UV work in UV space alone; the volume modes need the mesh (a [`crate::mesh::SurfaceMap`]) and are data
//! only in this version: preparing one reports [`NotComputable::NotImplemented`].
//!
//! [`view`] maps between the screen and the mesh for one camera ([`ViewProjector`]) and places images on screen
//! ([`ScreenImageTransform`], [`ScreenProjector`]) for projection painting and stencils.
//!
//! [`sampling`] samples images with a [`Filter`] and [`Repeat`] from a prefiltered level chain ([`ImageLevels`]).
//!
//! Design: `MaterialCraft-docs/requirements/projections.md` (section 3); decisions in ADR 0006–0008.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod data;
mod frame;
mod modes;
mod placement;
mod projector;
pub mod sampling;
#[cfg(test)]
mod tests;
pub mod view;

pub use data::{Filter, ImagePlacement, Projection, Repeat, SizeMode};
pub use frame::SceneFrame;
pub use modes::{Cylindrical, FacingFalloff, Falloff, Planar, ProjectionMode, Spherical, TriPlanar, UnknownMode, Volume, Warp, WarpGrid, WarpPoint};
pub use placement::UvPlacement;
pub use projector::{NotComputable, Prepared, Projector, SurfacePoint, Tap, Taps};
pub use sampling::{ImageLevels, LevelChain, Texels};
pub use view::{CpuViewProjector, ScreenImageTransform, ScreenPoint, ScreenProjector, SurfaceHit, ViewProjector, ViewScene};
