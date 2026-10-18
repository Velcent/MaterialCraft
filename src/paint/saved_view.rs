//! [`SavedView`]: how a document was last looked at (ADR 0008 mc2-vq15: the project keeps its cameras, the
//! environment choice and the exposure; every other display setting is a user default). Display state only:
//! nothing computed from the document depends on it, and a document without one opens with the user's defaults.

pub use super::saved_camera::{CameraOrigin, CameraValues, SavedCamera};

/// The lighting environment the view uses (EN-01…EN-05).
#[derive(Debug, Clone, PartialEq)]
pub struct SavedEnvironment {
    /// Which environment: [`SavedEnvironment::STUDIO`] (the built-in studio) or [`SavedEnvironment::MAP`] (the
    /// environment map at `map`). Other ids (from newer versions) are kept and shown as the studio.
    pub kind: String,
    /// The environment map's library path (`/`-separated, relative to a library root, as the asset tray lists
    /// it) when `kind` is [`SavedEnvironment::MAP`].
    pub map: Option<String>,
    /// Key light direction (radians), relative to the camera when `light_follows_camera`. For an environment map
    /// the azimuth is its turn and `light_follows_camera` its alignment (turning with the camera).
    pub light_yaw: f32,
    pub light_pitch: f32,
    pub light_intensity: f32,
    pub light_follows_camera: bool,
    /// Strength of the studio's ambient light.
    pub ambient: f32,
    /// The environment's exposure in stops (0 = as authored).
    pub exposure_stops: f32,
    /// How much of the environment shows behind the model (0 = the plain backdrop) and how blurred.
    pub background_opacity: f32,
    pub background_blur: f32,
}

impl SavedEnvironment {
    pub const STUDIO: &'static str = "studio";
    pub const MAP: &'static str = "map";

    /// The environment map's library path, if this environment is one.
    pub fn map_path(&self) -> Option<&str> {
        if self.kind == Self::MAP { self.map.as_deref() } else { None }
    }
}

impl Default for SavedEnvironment {
    fn default() -> SavedEnvironment {
        SavedEnvironment {
            kind: Self::STUDIO.to_string(),
            map: None,
            light_yaw: 0.9,
            light_pitch: 0.7,
            light_intensity: 3.0,
            light_follows_camera: true,
            ambient: 1.0,
            exposure_stops: 0.0,
            background_opacity: 0.0,
            background_blur: 0.3,
        }
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
