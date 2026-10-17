//! Orbit camera: the [`Camera`] the viewport draws a [`View`] with, and the matrices the shaders take from it.
use crate::camera::{Camera, Lens, Mat4, look_along};
use crate::render3d::View;

/// The viewport's lens: 40° vertical field of view, near and far planes around the unit-size model.
const LENS: Lens = Lens::Perspective { fov_y: 40.0 * std::f32::consts::PI / 180.0, near: 0.03, far: 60.0 };

pub(super) struct CamOut {
    pub vp: Mat4,
    pub eye: [f32; 3],
}

impl CamOut {
    pub fn of(c: &Camera, eye: [f32; 3]) -> CamOut {
        CamOut { vp: c.view_projection(), eye }
    }
}

/// The camera that draws `v` into a viewport of `viewport` units (pixels or UI points; only the aspect ratio
/// matters to the image, the units are those of screen positions on the camera). The model is the one placed by
/// its unit-size fit, as the viewport draws it.
pub fn view_camera(v: &View, viewport: [f32; 2]) -> Camera {
    let (eye, right, up, fwd) = basis(v);
    let aspect = viewport[0] / viewport[1];
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
    Camera::new(look_along(eye, right, up, fwd), LENS.matrix(aspect), viewport)
}

/// Eye position and the right, up and forward axes of the orbit camera.
fn basis(v: &View) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 3]) {
    use crate::v3::*;
    let (sy, cy) = v.yaw.sin_cos();
    let (sp, cp) = v.pitch.sin_cos();
    let eye0 = [v.dist * sy * cp, v.dist * sp, v.dist * cy * cp];
    let fwd = norm(mul(eye0, -1.0));
    let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
    let up = cross(right, fwd);
    let eye = add(eye0, add(mul(right, v.pan_x), mul(up, v.pan_y)));
    (eye, right, up, fwd)
}

pub(super) fn camera(v: &View, aspect: f32) -> CamOut {
    let c = view_camera(v, [aspect, 1.0]);
    CamOut::of(&c, basis(v).0)
}
