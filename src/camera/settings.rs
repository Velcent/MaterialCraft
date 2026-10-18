//! [`CameraSettings`]: the user-facing settings of a viewport camera (CM-03, CM-09): perspective or orthographic
//! projection, the field of view (or the equivalent focal length) and depth of field. Together with an
//! [`Orbit`] pose they give the [`Camera`] a frame is drawn with.

use super::lens::{Lens, focal_length, fov_y};
use super::mat4::look_along;
use super::orbit::Orbit;
use super::snapshot::Camera;

/// How a camera projects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Projection {
    #[default]
    Perspective,
    /// Parallel projection. Its height follows the orbit distance, so zooming works and switching keeps the
    /// framing of the pivot plane.
    Orthographic,
}

impl Projection {
    /// The id stored in files and settings.
    pub fn id(self) -> &'static str {
        match self {
            Projection::Perspective => "perspective",
            Projection::Orthographic => "orthographic",
        }
    }

    pub fn from_id(id: &str) -> Option<Projection> {
        match id {
            "perspective" => Some(Projection::Perspective),
            "orthographic" => Some(Projection::Orthographic),
            _ => None,
        }
    }
}

/// Depth of field (PP-04): off by default; when on, the image is sharp at `focus_distance` and blurs with
/// distance from it, more with a larger `aperture`. Drawn by accumulating frames through points of the lens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthOfField {
    pub enabled: bool,
    /// Distance from the eye to the sharp plane, in world units.
    pub focus_distance: f32,
    /// Radius of the lens opening, in world units (the model is about unit size).
    pub aperture: f32,
}

impl Default for DepthOfField {
    fn default() -> DepthOfField {
        DepthOfField { enabled: false, focus_distance: Orbit::default().distance, aperture: 0.03 }
    }
}

/// A viewport camera's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraSettings {
    pub projection: Projection,
    /// Vertical field of view in radians (perspective; it also sets the orthographic height at the pivot).
    pub fov_y: f32,
    pub depth_of_field: DepthOfField,
}

impl Default for CameraSettings {
    /// A 40° perspective camera (about 33 mm), no depth of field.
    fn default() -> CameraSettings {
        CameraSettings { projection: Projection::Perspective, fov_y: 40f32.to_radians(), depth_of_field: DepthOfField::default() }
    }
}

impl CameraSettings {
    /// Near and far clipping distances around the unit-size model.
    pub const NEAR: f32 = 0.03;
    pub const FAR: f32 = 60.0;
    /// The field of view the settings allow, in radians (about 5° … 120°).
    pub const FOV_RANGE: (f32, f32) = (0.087, 2.094);

    /// The focal length in millimetres equivalent to the field of view (through [`super::SENSOR_HEIGHT_MM`]).
    pub fn focal_length(&self) -> f32 {
        focal_length(self.fov_y)
    }

    /// Sets the field of view from a focal length in millimetres (kept within [`CameraSettings::FOV_RANGE`]).
    pub fn set_focal_length(&mut self, mm: f32) {
        if mm.is_finite() && mm > 0.0 {
            self.set_fov_y(fov_y(mm));
        }
    }

    /// Sets the vertical field of view in radians (kept within [`CameraSettings::FOV_RANGE`]).
    pub fn set_fov_y(&mut self, fov: f32) {
        if fov.is_finite() {
            self.fov_y = fov.clamp(Self::FOV_RANGE.0, Self::FOV_RANGE.1);
        }
    }

    /// The lens for an orbit at `distance` from its pivot.
    pub fn lens(&self, distance: f32) -> Lens {
        match self.projection {
            Projection::Perspective => Lens::Perspective { fov_y: self.fov_y, near: Self::NEAR, far: Self::FAR },
            Projection::Orthographic => Lens::Orthographic { height: 2.0 * distance * (0.5 * self.fov_y).tan(), near: Self::NEAR, far: Self::FAR },
        }
    }

    /// The camera drawing `orbit` through these settings into a viewport of `viewport` units.
    pub fn camera(&self, orbit: &Orbit, viewport: [f32; 2]) -> Camera {
        let b = orbit.basis();
        let aspect = viewport[0] / viewport[1];
        let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.0 };
        Camera::new(look_along(b.eye, b.right, b.up, b.forward), self.lens(orbit.distance).matrix(aspect), viewport)
    }

    /// True when depth of field is on and can be drawn (a parallel projection has no lens to blur through).
    pub fn blurs(&self) -> bool {
        let d = &self.depth_of_field;
        d.enabled && self.projection == Projection::Perspective && d.aperture > 0.0 && d.focus_distance > 0.0
    }
}
