//! The [`Camera`] the viewport draws a [`View`] with, the camera of the 2D (UV) view, and the matrices the shaders
//! take from them.
use crate::camera::{Camera, CameraSettings, Lens, Mat4};
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

/// The camera of the 2D (UV) view: a parallel projection looking at UV space laid out as the 2D view draws it
/// (world x = u, y = −v, z = 0: v runs down the texture, as image rows do), centred on `centre` (u, v) and showing
/// `height` UV units vertically on a viewport of `viewport` units.
pub fn uv_camera(centre: [f32; 2], height: f32, viewport: [f32; 2]) -> Camera {
    let (u, v) = (centre[0], centre[1]);
    let lens = Lens::Orthographic { height, near: 0.5, far: 1.5 };
    Camera::looking_at([u, -v, 1.0], [u, -v, 0.0], [0.0, 1.0, 0.0], &lens, viewport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uv_camera_puts_texels_where_the_2d_view_expects_them() {
        // 1 UV unit over 200 units of a 400 × 200 view, centred on (0.5, 0.5): u 0 … 1 spans x 100 … 300
        let c = uv_camera([0.5, 0.5], 1.0, [400.0, 200.0]);
        for (uv, screen) in [([0.0, 0.0], [100.0, 0.0]), ([1.0, 1.0], [300.0, 200.0]), ([0.25, 0.75], [150.0, 150.0])] {
            let p = c.project([uv[0], -uv[1], 0.0]).expect("in front");
            assert!((p.screen[0] - screen[0]).abs() < 1e-3 && (p.screen[1] - screen[1]).abs() < 1e-3, "{uv:?} → {:?}", p.screen);
            assert!(p.depth > 0.0 && p.depth < 1.0);
        }
    }
}
