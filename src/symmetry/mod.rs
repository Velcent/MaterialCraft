//! Symmetry on the mesh (PT-36 … PT-40): which symmetric copies a stroke or a fill makes, as plain data
//! ([`SymmetrySpec`]) and as transforms of object-space positions and normals ([`CopyTransform`]).
//!
//! Shared by the stamp stage (brush strokes repeat each stamp at its symmetric places) and fill projections (PT-40:
//! the volume-based projections repeat the same way), so it lives in its own module rather than in either.
//!
//! - [`SymmetrySpec`]: a mirror plane across an object axis at an offset, or radial copies around an object axis
//!   through a point (2 … 64 copies, alternate copies optionally flipped in U or V).
//! - [`CopyTransform`]: one copy: a rigid transform of positions (rotation or reflection plus translation) and the
//!   flips of the copy's stamp in its own frame.
//! - [`guides`]: geometry for the viewport's visual aids (PT-39): the mirror plane within the mesh's bounds, the
//!   radial axis, and where either meets the mesh.
//!
//! Design: `MaterialCraft-docs/requirements/painting-tools.md` section 6.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod copy;
pub mod guides;
mod spec;
#[cfg(test)]
mod tests;

pub use copy::CopyTransform;
pub use spec::{Flip, MAX_RADIAL_COPIES, SymmetrySpec, repeat};
