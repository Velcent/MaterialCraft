//! [`SceneCamera`]: a camera as other programs store it (glTF cameras, the cameras of Painter projects): a free pose
//! (eye, viewing and up direction), a lens and an optional frame aspect ratio, in the coordinates of the mesh it came
//! with. [`SceneCamera::placed`] moves it into the viewport's fit of that mesh, [`SceneCamera::view`] turns it into
//! the orbit pose and settings the viewport draws with.

use super::Vec3;
use super::orbit::Orbit;
use super::settings::{CameraSettings, Projection};
use crate::v3::{add, cross, len, mul};

/// How a scene camera projects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneLens {
    /// Vertical field of view in radians.
    Perspective { fov_y: f32 },
    /// Height of the view in world units.
    Orthographic { height: f32 },
}

/// A camera of an imported scene.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneCamera {
    pub name: String,
    pub eye: Vec3,
    /// Viewing direction (unit length).
    pub forward: Vec3,
    /// Up direction (unit length, at right angles to `forward`).
    pub up: Vec3,
    pub lens: SceneLens,
    /// Width / height of the camera's frame, when the file sets one.
    pub aspect: Option<f32>,
}

/// Distance an orthographic camera is placed in front of the fitted model's centre plane: the fit has radius 1, so
/// the whole model lies beyond the near plane.
const ORTHO_DISTANCE: f32 = 1.5;
/// Field of view an orthographic camera's height is expressed through when the distance allows it.
const ORTHO_FOV: f32 = 0.6981317; // 40°

impl SceneCamera {
    /// A camera given by a rotation quaternion `[x, y, z, w]` that turns the camera's own axes (looking down −z,
    /// +y up, as in glTF and OpenGL) into the scene. `None` for a zero or non-finite quaternion.
    pub fn from_rotation(name: &str, eye: Vec3, rotation: [f32; 4], lens: SceneLens, aspect: Option<f32>) -> Option<SceneCamera> {
        let n = rotation.iter().map(|v| v * v).sum::<f32>().sqrt();
        if !(n.is_finite() && n > 1e-12) {
            return None;
        }
        let q = rotation.map(|v| v / n);
        let forward = rotate(q, [0.0, 0.0, -1.0]);
        let up = rotate(q, [0.0, 1.0, 0.0]);
        Some(SceneCamera { name: name.to_string(), eye, forward, up, lens, aspect })
    }

    /// The rotation quaternion `[x, y, z, w]` of [`SceneCamera::from_rotation`] (for writing glTF).
    pub fn rotation(&self) -> [f32; 4] {
        let z = mul(self.forward, -1.0);
        let x = cross(self.up, z);
        let m = [x, self.up, z]; // columns
        let (m00, m11, m22) = (m[0][0], m[1][1], m[2][2]);
        let trace = m00 + m11 + m22;
        let q = if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            [(m[1][2] - m[2][1]) / s, (m[2][0] - m[0][2]) / s, (m[0][1] - m[1][0]) / s, 0.25 * s]
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            [0.25 * s, (m[1][0] + m[0][1]) / s, (m[2][0] + m[0][2]) / s, (m[1][2] - m[2][1]) / s]
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            [(m[1][0] + m[0][1]) / s, 0.25 * s, (m[2][1] + m[1][2]) / s, (m[2][0] - m[0][2]) / s]
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            [(m[2][0] + m[0][2]) / s, (m[2][1] + m[1][2]) / s, 0.25 * s, (m[0][1] - m[1][0]) / s]
        };
        let n = q.iter().map(|v| v * v).sum::<f32>().sqrt();
        if n.is_finite() && n > 0.0 { q.map(|v| v / n) } else { [0.0, 0.0, 0.0, 1.0] }
    }

    /// The camera moved like the mesh points of a fit: `p' = (p + translate) · scale` (directions unchanged, an
    /// orthographic height scaled).
    pub fn placed(&self, translate: Vec3, scale: f32) -> SceneCamera {
        let lens = match self.lens {
            SceneLens::Orthographic { height } => SceneLens::Orthographic { height: height * scale },
            other => other,
        };
        SceneCamera { eye: mul(add(self.eye, translate), scale), lens, ..self.clone() }
    }

    /// The orbit pose and settings that draw this camera's view (depth of field off, `None` for a degenerate pose).
    /// Perspective cameras keep their eye exactly; an orthographic camera's view does not depend on its eye's place
    /// along the viewing direction, so it moves to a distance in front of the fitted model that shows it whole and
    /// expresses its height through the field of view.
    pub fn view(&self) -> Option<(Orbit, CameraSettings)> {
        let mut orbit = Orbit::from_pose(self.eye, self.forward, self.up)?;
        let mut settings = CameraSettings::default();
        match self.lens {
            SceneLens::Perspective { fov_y } => settings.set_fov_y(fov_y),
            SceneLens::Orthographic { height } => {
                let height = if height.is_finite() && height > 0.0 { height } else { 2.0 * ORTHO_DISTANCE * (0.5 * ORTHO_FOV).tan() };
                orbit.distance = (0.5 * height / (0.5 * ORTHO_FOV).tan()).max(ORTHO_DISTANCE);
                settings.projection = Projection::Orthographic;
                settings.set_fov_y(2.0 * (0.5 * height / orbit.distance).atan());
            }
        }
        Some((orbit, settings))
    }

    /// The camera at an orbit pose with `settings` (the inverse of [`SceneCamera::view`]).
    pub fn of_view(name: &str, orbit: &Orbit, settings: &CameraSettings, aspect: Option<f32>) -> SceneCamera {
        let b = orbit.basis();
        let lens = match settings.projection {
            Projection::Perspective => SceneLens::Perspective { fov_y: settings.fov_y },
            Projection::Orthographic => SceneLens::Orthographic { height: 2.0 * orbit.distance * (0.5 * settings.fov_y).tan() },
        };
        SceneCamera { name: name.to_string(), eye: b.eye, forward: b.forward, up: b.up, lens, aspect }
    }

    /// True when the two cameras' poses and lenses agree within `eps` (positions, unit directions, radians and
    /// relative heights).
    pub fn close_to(&self, other: &SceneCamera, eps: f32) -> bool {
        let near = |a: Vec3, b: Vec3| len([a[0] - b[0], a[1] - b[1], a[2] - b[2]]) <= eps;
        let lens = match (self.lens, other.lens) {
            (SceneLens::Perspective { fov_y: a }, SceneLens::Perspective { fov_y: b }) => (a - b).abs() <= eps,
            (SceneLens::Orthographic { height: a }, SceneLens::Orthographic { height: b }) => (a - b).abs() <= eps * a.abs().max(1.0),
            _ => false,
        };
        let aspect = match (self.aspect, other.aspect) {
            (Some(a), Some(b)) => (a - b).abs() <= eps,
            (a, b) => a.is_none() && b.is_none(),
        };
        near(self.eye, other.eye) && near(self.forward, other.forward) && near(self.up, other.up) && lens && aspect && self.name == other.name
    }
}

/// `v` turned by the unit quaternion `q` = `[x, y, z, w]`.
fn rotate(q: [f32; 4], v: Vec3) -> Vec3 {
    let u = [q[0], q[1], q[2]];
    let w = q[3];
    let uv = cross(u, v);
    // v + 2w (u × v) + 2 u × (u × v)
    add(v, add(mul(uv, 2.0 * w), mul(cross(u, uv), 2.0)))
}
