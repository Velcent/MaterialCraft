//! [`SavedView`]: how a document was last looked at (ADR 0008 mc2-vq15: the project keeps its cameras, the
//! environment choice and the exposure; every other display setting is a user default). Display state only:
//! nothing computed from the document depends on it, and a document without one opens with the user's defaults.

use crate::camera::{CameraSettings, Orbit};

/// One camera of the document's camera list.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedCamera {
    pub name: String,
    /// The pose, as the orbit controller holds it.
    pub orbit: Orbit,
    /// Projection, field of view and depth of field.
    pub settings: CameraSettings,
}

impl Default for SavedCamera {
    /// The default camera (CM-01).
    fn default() -> SavedCamera {
        SavedCamera { name: SavedCamera::DEFAULT_NAME.to_string(), orbit: Orbit::default(), settings: CameraSettings::default() }
    }
}

impl SavedCamera {
    pub const DEFAULT_NAME: &'static str = "Default";
}

/// The lighting environment the view uses.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedEnvironment {
    /// Which environment: `studio` (the built-in analytic studio) is the only one so far; image environments come
    /// later and keep their own id here.
    pub kind: String,
    /// Key light direction (radians), relative to the camera when `light_follows_camera`.
    pub light_yaw: f32,
    pub light_pitch: f32,
    pub light_intensity: f32,
    pub light_follows_camera: bool,
    /// Strength of the environment's ambient light.
    pub ambient: f32,
}

impl SavedEnvironment {
    pub const STUDIO: &'static str = "studio";
}

impl Default for SavedEnvironment {
    fn default() -> SavedEnvironment {
        SavedEnvironment { kind: Self::STUDIO.to_string(), light_yaw: 0.9, light_pitch: 0.7, light_intensity: 3.0, light_follows_camera: true, ambient: 1.0 }
    }
}

/// The view stored with a document.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedView {
    /// The camera list; the first is the default camera.
    pub cameras: Vec<SavedCamera>,
    /// Index of the active camera (an index past the list means the first).
    pub active_camera: usize,
    /// Exposure in stops.
    pub exposure_stops: f32,
    pub environment: SavedEnvironment,
}

impl Default for SavedView {
    fn default() -> SavedView {
        SavedView { cameras: vec![SavedCamera::default()], active_camera: 0, exposure_stops: 0.0, environment: SavedEnvironment::default() }
    }
}

impl SavedView {
    /// The active camera (the first when the index is out of range; `None` only for an empty list).
    pub fn active(&self) -> Option<&SavedCamera> {
        self.cameras.get(self.active_camera).or_else(|| self.cameras.first())
    }
}
