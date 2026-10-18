//! `[display]`: the Paint viewport's display settings as user defaults (DS-02, ADR 0008 mc2-vq15). The view
//! preferences (tone curve, anti-aliasing, texture sharpness, height display, lit channels) follow every change;
//! the project part (camera, exposure, environment) is what new and imported documents start with, changed only
//! by "Use as defaults" (a document keeps its own). Shadows are view preferences. Enums are stored by id, so the
//! file stays hand-editable.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySettings {
    /// `filmic`, `linear` or `aces`.
    pub tone_curve: String,
    /// `off`, `standard` or `high`.
    pub antialiasing: String,
    /// Mip-map bias, 0 … -3.
    pub mip_bias: f32,
    /// Anisotropic filtering level; 0 = the most the GPU offers.
    pub anisotropy: f32,
    /// Channel and mesh-map views lit by the scene instead of unlit.
    pub lit_channels: bool,
    /// How height is shown: 0 normal map only, 1 bump, 2 parallax, 3 displacement.
    pub relief: u32,
    pub height_scale: f32,
    pub height_self_shadow: bool,
    pub normal_strength: f32,
    pub wireframe: bool,
    /// Scene shadows (EN-07, EN-08): on, opacity 0…1, and `smooth`, `balanced` or `fine`.
    pub shadows: bool,
    pub shadow_opacity: f32,
    pub shadow_quality: String,
    /// The project part's defaults.
    pub project: ProjectDisplayDefaults,
}

impl Default for DisplaySettings {
    fn default() -> DisplaySettings {
        DisplaySettings {
            tone_curve: "filmic".into(),
            antialiasing: "standard".into(),
            mip_bias: 0.0,
            anisotropy: 0.0,
            lit_channels: false,
            relief: 0,
            height_scale: 0.08,
            height_self_shadow: true,
            normal_strength: 1.0,
            wireframe: false,
            shadows: true,
            shadow_opacity: 1.0,
            shadow_quality: "smooth".into(),
            project: ProjectDisplayDefaults::default(),
        }
    }
}

/// What a document without a saved view starts with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectDisplayDefaults {
    /// `perspective` or `orthographic`.
    pub projection: String,
    pub fov_y_degrees: f32,
    pub depth_of_field: bool,
    pub aperture: f32,
    pub exposure_stops: f32,
    pub light_yaw: f32,
    pub light_pitch: f32,
    pub light_intensity: f32,
    pub light_follows_camera: bool,
    pub ambient: f32,
    /// The environment map's library path; empty for the built-in studio.
    pub environment_map: String,
    pub environment_exposure: f32,
    pub background_opacity: f32,
    pub background_blur: f32,
}

impl Default for ProjectDisplayDefaults {
    fn default() -> ProjectDisplayDefaults {
        ProjectDisplayDefaults {
            projection: "perspective".into(),
            fov_y_degrees: 40.0,
            depth_of_field: false,
            aperture: 0.03,
            exposure_stops: 0.0,
            light_yaw: 0.9,
            light_pitch: 0.7,
            light_intensity: 3.0,
            light_follows_camera: true,
            ambient: 1.0,
            environment_map: String::new(),
            environment_exposure: 0.0,
            background_opacity: 0.0,
            background_blur: 0.3,
        }
    }
}
