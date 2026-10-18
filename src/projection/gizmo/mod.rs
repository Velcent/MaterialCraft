//! Gizmo maths for editing projections in the view (PJ-23 … PJ-25): no drawing and no input handling, only the
//! geometry the GUI draws and the drags it turns pointer rays into, so every gizmo edit is testable without a window.
//!
//! - [`PlacedVolume`]: a [`super::Volume`] as a box in the mesh's object space and back (offset, Euler angles and
//!   scale as the projectors read them).
//! - [`Gizmo`]: the one transform gizmo of the volume modes (ADR 0008 mc2-q-pt-05c), in [`GizmoMode`] move, rotate
//!   or scale, along the volume's or the object's axes ([`Space`]); [`Gizmo::shapes`] gives its parts, [`hit`]
//!   puts them on screen and finds the one under the pointer.
//! - [`VolumeDrag`]: one drag of a [`Handle`], optionally in [`Steps`].
//! - [`snap_to_surface`]: the volume set onto a surface point, facing along its normal (planar and warp only).
//! - [`uv`]: the 2D placement handle of UV mode in UV coordinates.
//! - [`WarpGridShape`], [`WarpPointDrag`]: the warp grid's anchors and lines, and dragging an anchor.
//!
//! Spaces: everything 3D is in the mesh's object space, as [`super::ViewProjector`] picks and projects; rays come
//! from [`super::ViewProjector::ray`].

mod drag;
mod handles;
pub mod hit;
pub mod maths;
mod placed;
mod surface_snap;
#[cfg(test)]
mod tests;
pub mod uv;
mod warp_points;

pub use drag::{Steps, VolumeDrag};
pub use handles::{Gizmo, GizmoMode, Handle, HandleShape, Shape, ShapeKind, Space, axes};
pub use hit::{ScreenShape, pick, to_screen};
pub use maths::Ray;
pub use placed::PlacedVolume;
pub use surface_snap::snap_to_surface;
pub use uv::{UvDrag, UvFrame, UvHandle, UvShape};
pub use warp_points::{WarpGridShape, WarpPointDrag};

use super::view::ViewProjector;
use crate::mesh::Vec3;
use crate::v3::{len, sub};

/// The object-space length that spans `pixels` screen units across the view at `centre` (so the gizmo keeps its
/// size on screen however far the camera is); `None` when `centre` is not in front of the camera.
pub fn length_for_pixels(view: &dyn ViewProjector, centre: Vec3, pixels: f32) -> Option<f32> {
    let s = view.to_screen(centre)?;
    let through = view.ray(s.pos)?;
    let aside = view.ray([s.pos[0] + pixels, s.pos[1]])?;
    let hit = maths::hit_plane(&aside, centre, through.dir)?;
    let l = len(sub(hit, centre));
    (l.is_finite() && l > 0.0).then_some(l)
}
