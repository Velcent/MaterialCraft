//! [`SavedCamera`]: one camera of a document's camera list (CM-03, CM-07, CM-08, CM-12): its name, pose, settings
//! and frame aspect ratio, where it came from (made in MaterialCraft, or imported with the values it was imported
//! with, so an edited camera can be restored) and whether it is locked against edits.

use crate::camera::{CameraSettings, Orbit, SceneCamera};

/// A camera's pose and settings: everything an edit changes and a restore puts back.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CameraValues {
    /// The pose, as the orbit controller holds it (in the viewport's fit of the model).
    pub orbit: Orbit,
    /// Projection, field of view and depth of field.
    pub settings: CameraSettings,
    /// Width / height of the camera's frame; `None` = not set (the frame is the viewport).
    pub aspect: Option<f32>,
}

impl CameraValues {
    /// The values that draw `camera` (already placed in the viewport's fit); `None` for a degenerate pose.
    pub fn of_scene(camera: &SceneCamera) -> Option<CameraValues> {
        let (orbit, settings) = camera.view()?;
        Some(CameraValues { orbit, settings, aspect: camera.aspect.filter(|a| a.is_finite() && *a > 0.0) })
    }
}

/// Where a camera came from.
#[derive(Debug, Clone, PartialEq)]
pub enum CameraOrigin {
    /// Made in MaterialCraft (the default camera, views saved as cameras).
    Own,
    /// Imported from a file. `source` names the kind of file it came from (see [`CameraOrigin::SOURCES`]);
    /// `original` holds the values it was imported with.
    Imported { source: String, original: CameraValues },
}

impl CameraOrigin {
    /// Sources the importers write: a glTF file's cameras, a Painter project's saved viewer camera and its scene
    /// cameras. Other ids are kept as they are.
    pub const SOURCES: [&'static str; 3] = ["gltf", "painter-viewer", "painter-scene"];
}

/// One camera of the document's camera list.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedCamera {
    pub name: String,
    /// The pose, as the orbit controller holds it.
    pub orbit: Orbit,
    /// Projection, field of view and depth of field.
    pub settings: CameraSettings,
    /// Width / height of the camera's frame; `None` = not set.
    pub aspect: Option<f32>,
    pub origin: CameraOrigin,
    /// Edits (settings, "set from view", restore) are refused while locked (CM-08).
    pub locked: bool,
}

impl Default for SavedCamera {
    /// The default camera (CM-01).
    fn default() -> SavedCamera {
        SavedCamera::own(SavedCamera::DEFAULT_NAME, CameraValues::default())
    }
}

impl SavedCamera {
    pub const DEFAULT_NAME: &'static str = "Default";

    /// A camera made in MaterialCraft.
    pub fn own(name: &str, values: CameraValues) -> SavedCamera {
        SavedCamera { name: name.to_string(), orbit: values.orbit, settings: values.settings, aspect: values.aspect, origin: CameraOrigin::Own, locked: false }
    }

    /// A camera imported from `source` with `values`, which it also keeps as its original.
    pub fn imported(name: &str, source: &str, values: CameraValues) -> SavedCamera {
        let origin = CameraOrigin::Imported { source: source.to_string(), original: values };
        SavedCamera { origin, ..SavedCamera::own(name, values) }
    }

    pub fn values(&self) -> CameraValues {
        CameraValues { orbit: self.orbit, settings: self.settings, aspect: self.aspect }
    }

    /// Takes `values` unless the camera is locked; true when it took them.
    pub fn set_values(&mut self, values: CameraValues) -> bool {
        if self.locked {
            return false;
        }
        (self.orbit, self.settings, self.aspect) = (values.orbit, values.settings, values.aspect);
        true
    }

    /// The values the camera was imported with (`None` for own cameras).
    pub fn original(&self) -> Option<&CameraValues> {
        match &self.origin {
            CameraOrigin::Imported { original, .. } => Some(original),
            CameraOrigin::Own => None,
        }
    }

    /// True for an imported camera whose values differ from the imported ones (CM-07).
    pub fn is_modified(&self) -> bool {
        self.original().is_some_and(|o| *o != self.values())
    }

    /// Puts an imported camera back to its imported values (not while locked); true when something changed.
    pub fn restore(&mut self) -> bool {
        match self.original().copied() {
            Some(o) if o != self.values() => self.set_values(o),
            _ => false,
        }
    }
}
