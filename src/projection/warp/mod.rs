//! The warp projection (PJ-20, PJ-21): a grid of `rows × columns` anchors with Bézier handles forms a smooth
//! patch surface inside the projection volume; the image is laid on it (each anchor has an image coordinate) and
//! projected along the grid's local z, up to the projection depth on either side, with the depth falloff.
//!
//! - [`grid`]: the grid's conventions (grid space −1…1, row 0 at the top, automatic handles) and its edits;
//! - [`bezier`]: the patch surface ([`BezierMesh`], from PhotoCraft's `geom::warp`, generalised to 3D);
//! - [`surface`]: [`WarpSurface`], the surface with its image and its inverse ([`InverseLookup`] + Newton);
//! - [`prepared`]: [`PreparedWarp`], the per-texel projector, and the shared surface cache.
//!
//! Painter's warp grids are imported as they are stored and reported approximated (ADR 0008 mc2-q-pj-05: our own
//! warp, no pixel matching).

pub mod bezier;
pub mod grid;
mod lookup;
pub mod prepared;
pub mod surface;
#[cfg(test)]
mod tests;

pub use bezier::BezierMesh;
pub use grid::{GridError, MAX_SIDE};
pub use lookup::InverseLookup;
pub use prepared::{GRID_PER_LOCAL, PreparedWarp, surface_for};
pub use surface::{SurfaceHit, WarpSurface};
