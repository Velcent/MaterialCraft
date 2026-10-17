//! [`Camera`]: a snapshot of a camera for one frame (or one stored stroke), and the screen ↔ world maths on it.

use super::Vec3;
use super::lens::Lens;
use super::mat4::{IDENTITY, Mat4, inverse, look_at, multiply, transform_point};
use crate::v3::{dot, len, mul, norm, sub};

/// Below this clip-space w a point counts as behind the camera.
const MIN_W: f32 = 1e-9;

/// View and projection matrices and the viewport they are drawn into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// World → view space.
    pub view: Mat4,
    /// View → clip space.
    pub projection: Mat4,
    /// Width and height of the viewport, in the units screen positions use (pixels or UI points).
    pub viewport: [f32; 2],
}

impl Default for Camera {
    /// Identity matrices on a 1 × 1 viewport.
    fn default() -> Camera {
        Camera { view: IDENTITY, projection: IDENTITY, viewport: [1.0, 1.0] }
    }
}

/// A world point on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projected {
    /// Screen position (viewport units, origin top-left, y down).
    pub screen: [f32; 2],
    /// Window depth as OpenGL's depth buffer stores it: 0 at the near plane, 1 at the far plane.
    pub depth: f32,
    /// Distance in front of the camera along its viewing direction (world units).
    pub view_depth: f32,
}

/// A ray in world space from the near plane; `dir` has unit length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
    /// Distance along the ray from the near to the far plane: what lies beyond is not drawn.
    pub length: f32,
}

impl Ray {
    /// The point `origin + t · dir`.
    pub fn at(&self, t: f32) -> Vec3 {
        [self.origin[0] + t * self.dir[0], self.origin[1] + t * self.dir[1], self.origin[2] + t * self.dir[2]]
    }
}

impl Camera {
    pub fn new(view: Mat4, projection: Mat4, viewport: [f32; 2]) -> Camera {
        Camera { view, projection, viewport }
    }

    /// A camera at `eye` looking at `target` (`up` roughly up) through `lens`, for a viewport of `viewport` units.
    pub fn looking_at(eye: Vec3, target: Vec3, up: Vec3, lens: &Lens, viewport: [f32; 2]) -> Camera {
        Camera { view: look_at(eye, target, up), projection: lens.matrix(aspect(viewport)), viewport }
    }

    /// World → clip space.
    pub fn view_projection(&self) -> Mat4 {
        multiply(&self.projection, &self.view)
    }

    /// Width / height of the viewport (1 for an empty one).
    pub fn aspect(&self) -> f32 {
        aspect(self.viewport)
    }

    /// The lens of the projection matrix, if it is a plain perspective or orthographic one.
    pub fn lens(&self) -> Option<Lens> {
        Lens::of_matrix(&self.projection)
    }

    /// True for a parallel projection (clip w does not depend on depth).
    pub fn is_orthographic(&self) -> bool {
        self.projection[11] == 0.0 && self.projection[15] != 0.0
    }

    /// The camera position in world space (`None` if the view matrix is singular).
    pub fn eye(&self) -> Option<Vec3> {
        let inv = inverse(&self.view)?;
        Some([inv[12], inv[13], inv[14]])
    }

    /// The viewing direction in world space (unit length; view space −z).
    pub fn forward(&self) -> Vec3 {
        let v = &self.view;
        norm([-v[2], -v[6], -v[10]])
    }

    /// The unit direction from `world` towards the camera: towards the eye for a perspective camera, against the
    /// viewing direction for an orthographic one.
    pub fn towards_camera(&self, world: Vec3) -> Vec3 {
        self.transform().towards_camera(world)
    }

    /// Where `world` lands on screen, or `None` when it is behind the camera (or the matrices are not finite).
    /// Points outside the viewport or beyond the near and far planes are still projected. Many points: use
    /// [`Camera::transform`] once.
    pub fn project(&self, world: Vec3) -> Option<Projected> {
        self.transform().project(world)
    }

    /// The ray through screen position `screen`, starting on the near plane. `None` if the matrices cannot be
    /// inverted.
    pub fn ray(&self, screen: [f32; 2]) -> Option<Ray> {
        self.transform().ray(screen)
    }

    /// The camera with its combined matrices computed once, for projecting many points.
    pub fn transform(&self) -> CameraTransform {
        let vp = self.view_projection();
        CameraTransform { camera: *self, inv_vp: inverse(&vp), vp, eye: self.eye(), orthographic: self.is_orthographic(), forward: self.forward() }
    }

    /// The distance in front of the camera of a window depth (0 … 1) from this camera's projection, the inverse
    /// of [`Projected::depth`]. `None` where the projection cannot map back.
    pub fn view_depth_of(&self, depth: f32) -> Option<f32> {
        let p = &self.projection;
        let ndc = 2.0 * depth - 1.0;
        let den = p[10] - ndc * p[11];
        let z = (ndc * p[15] - p[14]) / den;
        (den != 0.0 && z.is_finite()).then_some(-z)
    }

    /// Screen position of normalised device coordinates (−1 … 1, y up).
    pub fn ndc_to_screen(&self, ndc: [f32; 2]) -> [f32; 2] {
        [(ndc[0] * 0.5 + 0.5) * self.viewport[0], (0.5 - ndc[1] * 0.5) * self.viewport[1]]
    }

    /// Normalised device coordinates (−1 … 1, y up) of a screen position.
    pub fn screen_to_ndc(&self, screen: [f32; 2]) -> [f32; 2] {
        let (w, h) = (nonzero(self.viewport[0]), nonzero(self.viewport[1]));
        [screen[0] / w * 2.0 - 1.0, 1.0 - screen[1] / h * 2.0]
    }

    /// The same camera on a viewport of another size (the projection keeps its matrix; positions scale with the
    /// viewport, as when going from UI points to pixels).
    pub fn with_viewport(&self, viewport: [f32; 2]) -> Camera {
        Camera { viewport, ..*self }
    }

    /// Screen units per world unit at view depth `view_depth` (vertically), for sizes on screen.
    pub fn screen_scale(&self, view_depth: f32) -> f32 {
        let p = &self.projection;
        let per_ndc = 0.5 * self.viewport[1] * p[5];
        if self.is_orthographic() { per_ndc } else { per_ndc / view_depth.max(1e-9) }
    }

    /// Dot product of a unit `normal` with the direction towards the camera at `world`.
    pub fn facing(&self, world: Vec3, normal: Vec3) -> f32 {
        dot(norm(normal), self.towards_camera(world))
    }
}

/// A [`Camera`] with its combined and inverse matrices computed once: the per-point path of projectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTransform {
    pub camera: Camera,
    vp: Mat4,
    inv_vp: Option<Mat4>,
    eye: Option<Vec3>,
    orthographic: bool,
    forward: Vec3,
}

impl CameraTransform {
    /// See [`Camera::project`].
    pub fn project(&self, world: Vec3) -> Option<Projected> {
        let clip = transform_point(&self.vp, world);
        if clip[3].is_nan() || clip[3] <= MIN_W {
            return None;
        }
        let ndc = [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]];
        let view_z = transform_point(&self.camera.view, world)[2];
        let p = Projected { screen: self.camera.ndc_to_screen([ndc[0], ndc[1]]), depth: 0.5 * ndc[2] + 0.5, view_depth: -view_z };
        (p.screen.iter().all(|v| v.is_finite()) && p.depth.is_finite()).then_some(p)
    }

    /// See [`Camera::ray`].
    pub fn ray(&self, screen: [f32; 2]) -> Option<Ray> {
        let inv = self.inv_vp.as_ref()?;
        let [x, y] = self.camera.screen_to_ndc(screen);
        let unproject = |z: f32| {
            let h = transform_point(inv, [x, y, z]);
            (h[3].abs() > MIN_W).then(|| [h[0] / h[3], h[1] / h[3], h[2] / h[3]])
        };
        let (near, far) = (unproject(-1.0)?, unproject(1.0)?);
        let d = sub(far, near);
        let l = len(d);
        (l.is_finite() && l > 0.0).then(|| Ray { origin: near, dir: mul(d, 1.0 / l), length: l })
    }

    /// See [`Camera::towards_camera`].
    pub fn towards_camera(&self, world: Vec3) -> Vec3 {
        match (self.orthographic, self.eye) {
            (false, Some(eye)) => norm(sub(eye, world)),
            _ => mul(self.forward, -1.0),
        }
    }

    /// The camera position (`None` for a singular view matrix).
    pub fn eye(&self) -> Option<Vec3> {
        self.eye
    }

    pub fn is_orthographic(&self) -> bool {
        self.orthographic
    }
}

fn aspect(viewport: [f32; 2]) -> f32 {
    let a = viewport[0] / viewport[1];
    if a.is_finite() && a > 0.0 { a } else { 1.0 }
}

fn nonzero(v: f32) -> f32 {
    if v.abs() > 0.0 && v.is_finite() { v } else { 1.0 }
}
