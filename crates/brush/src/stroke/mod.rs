//! Input transforms in front of a [`crate::StrokeSession`] (roadmap step 30): what the pointer gives becomes the
//! points a stroke is painted (and recorded) with.
//!
//! - [`LazyMouse`]: the brush trails the pointer on a string (PT-34, PT-35; PhotoCraft's pulled-string smoother
//!   through the engine adapter). Runs in the stroke's frame: screen units for 3D-view strokes.
//! - [`line`]: straight strokes from the previous stroke's end (PT-31 … PT-33): [`LineAnchor`] keeps that end as a
//!   surface point, so the line starts there again after the camera moved; [`line_points`] and [`snap_angle`]
//!   make the stroke.
//!
//! Strokes record the transformed points, so replays need none of this.

mod lazy;
pub mod line;

pub use lazy::LazyMouse;
pub use line::{LineAnchor, line_points, snap_angle};
