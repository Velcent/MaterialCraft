//! The [`Camera`] the viewport draws a [`View`] with, and the matrices the shaders take from it.
use crate::camera::{Camera, CameraSettings, Mat4};
use crate::render3d::View;

pub(super) struct CamOut {
    pub vp: Mat4,
    pub eye: [f32; 3],
    /// Projection elements 10, 11, 14 and 15: what turns window depth back into view distance.
    pub depth_lin: [f32; 4],
}

impl CamOut {
    pub fn of(c: &Camera) -> CamOut {
        let p = &c.projection;
        // a camera from an orbit always has an invertible view matrix; the origin is a harmless fallback
        CamOut { vp: c.view_projection(), eye: c.eye().unwrap_or([0.0; 3]), depth_lin: [p[10], p[11], p[14], p[15]] }
    }
}

/// The camera that draws `v` into a viewport of `viewport` units (pixels or UI points; only the aspect ratio
/// matters to the image, the units are those of screen positions on the camera) with the default camera
/// settings (a 40° perspective). The model is the one placed by its unit-size fit, as the viewport draws it.
pub fn view_camera(v: &View, viewport: [f32; 2]) -> Camera {
    v.camera(&CameraSettings::default(), viewport)
}
