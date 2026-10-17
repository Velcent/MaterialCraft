//! [`Lens`]: how a camera projects, as settings, and its projection matrix. Field of view and focal length are two
//! views of the same setting, converted through one sensor size.

use super::mat4::Mat4;

/// The sensor height every focal length refers to: a 35 mm full-frame sensor (36 × 24 mm), measured vertically.
pub const SENSOR_HEIGHT_MM: f32 = 24.0;

/// The focal length in millimetres that gives the vertical field of view `fov_y` (radians) on the sensor.
pub fn focal_length(fov_y: f32) -> f32 {
    0.5 * SENSOR_HEIGHT_MM / (0.5 * fov_y).tan()
}

/// The vertical field of view (radians) of a lens with focal length `mm` on the sensor.
pub fn fov_y(mm: f32) -> f32 {
    2.0 * (0.5 * SENSOR_HEIGHT_MM / mm).atan()
}

/// A camera's projection. Distances are in world units along the viewing direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lens {
    /// A pinhole camera with the vertical field of view `fov_y` (radians).
    Perspective { fov_y: f32, near: f32, far: f32 },
    /// A parallel projection showing `height` world units vertically.
    Orthographic { height: f32, near: f32, far: f32 },
}

impl Lens {
    /// A perspective lens given by its focal length in millimetres.
    pub fn with_focal_length(mm: f32, near: f32, far: f32) -> Lens {
        Lens::Perspective { fov_y: fov_y(mm), near, far }
    }

    /// The projection matrix (view → clip space, OpenGL depth range) for a viewport `aspect` = width / height.
    /// Degenerate settings (zero or negative sizes and ranges) are clamped to tiny positive values, so the matrix
    /// stays invertible.
    pub fn matrix(&self, aspect: f32) -> Mat4 {
        let aspect = positive(aspect, 1.0);
        match *self {
            Lens::Perspective { fov_y, near, far } => {
                let near = positive(near, 1e-3);
                let far = if far.is_finite() && far > near { far } else { near * 1e6 };
                let t = 1.0 / positive((0.5 * fov_y).tan(), 1e-6);
                let (a, b) = (-(far + near) / (far - near), -2.0 * far * near / (far - near));
                [t / aspect, 0.0, 0.0, 0.0, 0.0, t, 0.0, 0.0, 0.0, 0.0, a, -1.0, 0.0, 0.0, b, 0.0]
            }
            Lens::Orthographic { height, near, far } => {
                let h = positive(height, 1e-6);
                let far = if far.is_finite() && far > near { far } else { near + 1e6 };
                let (a, b) = (-2.0 / (far - near), -(far + near) / (far - near));
                [2.0 / (h * aspect), 0.0, 0.0, 0.0, 0.0, 2.0 / h, 0.0, 0.0, 0.0, 0.0, a, 0.0, 0.0, 0.0, b, 1.0]
            }
        }
    }

    /// The lens behind a projection matrix made like [`Lens::matrix`] (also by other programs: OpenGL-style
    /// `perspective` and `ortho` without an off-centre shift), or `None` if the matrix is neither.
    pub fn of_matrix(m: &Mat4) -> Option<Lens> {
        let [m0, _, _, _, _, m5, _, _, _, _, m10, m11, _, _, m14, m15] = *m;
        if !m.iter().all(|v| v.is_finite()) || m0 == 0.0 || m5 == 0.0 {
            return None;
        }
        if m11 == -1.0 && m15 == 0.0 {
            let fov_y = 2.0 * (1.0 / m5).atan();
            let (near, far) = (m14 / (m10 - 1.0), m14 / (m10 + 1.0));
            return (near > 0.0 && far > near).then_some(Lens::Perspective { fov_y, near, far });
        }
        // m10 < 0: the far plane lies beyond the near one (view space looks down −z)
        if m11 == 0.0 && m15 == 1.0 && m10 < 0.0 {
            let height = 2.0 / m5;
            let (near, far) = ((m14 + 1.0) / m10, (m14 - 1.0) / m10);
            return Some(Lens::Orthographic { height, near, far });
        }
        None
    }

    /// The width / height ratio a matrix from [`Lens::matrix`] was made for.
    pub fn aspect_of(m: &Mat4) -> Option<f32> {
        let a = m[5] / m[0];
        (a.is_finite() && a > 0.0).then_some(a)
    }
}

fn positive(v: f32, fallback: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v } else { fallback }
}
