//! Cameras as data: a [`Camera`] is a snapshot of view and projection matrices plus the size of the viewport it is
//! drawn into, with the maths every screen ↔ world mapping needs (projecting points, rays through screen points).
//! [`Lens`] describes the projection (perspective or orthographic) and builds its matrix; [`focal_length`] and
//! [`fov_y`] convert between a vertical field of view and a focal length through one sensor size
//! ([`SENSOR_HEIGHT_MM`]).
//!
//! Conventions (OpenGL, as `gpu3d` draws): right-handed, the camera looks down −z in view space, matrices are
//! column-major ([`Mat4`]), clip space z runs −1 (near) … 1 (far), window depth 0 … 1. Screen positions are in the
//! viewport's units (pixels or UI points) with the origin at the viewport's top-left corner, y down.
//!
//! The camera is a snapshot: whatever moves it (the viewport's orbit navigation today; an orbit controller and camera
//! sets later) produces a new [`Camera`] each frame, for example through [`Camera::looking_at`].
//!
//! Std only; no dependency besides `v3`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod lens;
mod mat4;
mod snapshot;
#[cfg(test)]
mod tests;

pub use lens::{Lens, SENSOR_HEIGHT_MM, focal_length, fov_y};
pub use mat4::{IDENTITY, Mat4, inverse, look_along, look_at, multiply, transform_point};
pub use snapshot::{Camera, CameraTransform, Projected, Ray};

/// A point or direction in world space.
pub type Vec3 = [f32; 3];
