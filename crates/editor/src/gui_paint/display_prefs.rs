//! The viewport's display settings ↔ the user's defaults in `config.toml` (`[display]`, ADR 0008 mc2-vq15).
//! View preferences are written back whenever they change; the project part (camera, exposure, environment)
//! only seeds documents without a saved view and changes only through "Use as defaults".

use super::view_state::ViewState;
use crate::gui_settings::UserSettings;
use crate::gui_settings::display::{DisplaySettings as Prefs, ProjectDisplayDefaults};
use materialcraft::camera::{CameraSettings, Orbit, Projection};
use materialcraft::gpu3d::{Antialiasing, GpuOpts, ShadowQuality, ToneCurve};
use materialcraft::paint::saved_view::SavedEnvironment;

/// A view state from the user's defaults (a document's own view is applied on top when it has one).
pub fn state_from(p: &Prefs) -> ViewState {
    let mut s = ViewState::default();
    let d = &mut s.display;
    d.tone_curve = ToneCurve::from_id(&p.tone_curve).unwrap_or_default();
    d.antialiasing = Antialiasing::from_id(&p.antialiasing).unwrap_or_default();
    d.mip_bias = if p.mip_bias.is_finite() { p.mip_bias.clamp(-3.0, 0.0) } else { 0.0 };
    s.lit_channels = p.lit_channels;
    let v = &mut s.view;
    v.relief = p.relief.min(3);
    v.height_scale = finite(p.height_scale, v.height_scale).clamp(0.0, 0.5);
    v.normal_amt = finite(p.normal_strength, v.normal_amt).clamp(0.0, 1.0);
    v.wireframe = p.wireframe;
    let sh = &mut s.display.shadows;
    sh.enabled = p.shadows;
    sh.opacity = finite(p.shadow_opacity, 1.0).clamp(0.0, 1.0);
    sh.quality = ShadowQuality::from_id(&p.shadow_quality).unwrap_or_default();
    apply_project(&mut s, &p.project);
    s
}

/// The GPU options from the user's defaults.
pub fn opts_from(p: &Prefs) -> GpuOpts {
    GpuOpts { self_shadow: p.height_self_shadow, aniso: finite(p.anisotropy, 0.0).clamp(0.0, 16.0) }
}

/// Puts the project defaults into `s`: a fresh default camera, exposure and the environment.
pub fn apply_project(s: &mut ViewState, p: &ProjectDisplayDefaults) {
    let mut camera = CameraSettings { projection: Projection::from_id(&p.projection).unwrap_or_default(), ..CameraSettings::default() };
    camera.set_fov_y(finite(p.fov_y_degrees, 40.0).to_radians());
    camera.depth_of_field.enabled = p.depth_of_field;
    camera.depth_of_field.aperture = finite(p.aperture, camera.depth_of_field.aperture).max(0.0);
    let mut saved = s.saved();
    saved.cameras = vec![materialcraft::paint::saved_view::SavedCamera { settings: camera, orbit: Orbit::default(), ..Default::default() }];
    saved.active_camera = 0;
    saved.exposure_stops = finite(p.exposure_stops, 0.0);
    let e = &mut saved.environment;
    (e.light_yaw, e.light_pitch, e.light_intensity, e.light_follows_camera, e.ambient) =
        (finite(p.light_yaw, 0.9), finite(p.light_pitch, 0.7), finite(p.light_intensity, 3.0), p.light_follows_camera, finite(p.ambient, 1.0));
    let map = p.environment_map.trim();
    (e.kind, e.map) = if map.is_empty() { (SavedEnvironment::STUDIO.into(), None) } else { (SavedEnvironment::MAP.into(), Some(map.to_string())) };
    (e.exposure_stops, e.background_opacity, e.background_blur) =
        (finite(p.environment_exposure, 0.0), finite(p.background_opacity, 0.0).clamp(0.0, 1.0), finite(p.background_blur, 0.3).clamp(0.0, 1.0));
    s.apply_saved(&saved);
}

/// The project part of `s` as defaults for new documents.
pub fn project_of(s: &ViewState) -> ProjectDisplayDefaults {
    let (c, v) = (&s.display.camera, &s.view);
    ProjectDisplayDefaults {
        projection: c.projection.id().into(),
        fov_y_degrees: c.fov_y.to_degrees(),
        depth_of_field: c.depth_of_field.enabled,
        aperture: c.depth_of_field.aperture,
        exposure_stops: s.display.exposure_stops,
        light_yaw: v.light_yaw,
        light_pitch: v.light_pitch,
        light_intensity: v.light_intensity,
        light_follows_camera: v.light_follows_camera,
        ambient: v.ambient,
        environment_map: s.environment_map().unwrap_or_default().to_string(),
        environment_exposure: s.display.environment.exposure_stops,
        background_opacity: s.display.environment.backdrop.opacity,
        background_blur: s.display.environment.backdrop.blur,
    }
}

/// Writes the view preferences of `s` and `opts` to the user settings when they changed (the project part stays).
pub fn sync(settings: &mut UserSettings, s: &ViewState, opts: &GpuOpts) {
    let d = &s.display;
    let current = &settings.get().display;
    let want = Prefs {
        tone_curve: d.tone_curve.id().into(),
        antialiasing: d.antialiasing.id().into(),
        mip_bias: d.mip_bias,
        anisotropy: opts.aniso,
        lit_channels: s.lit_channels,
        relief: s.view.relief,
        height_scale: s.view.height_scale,
        height_self_shadow: opts.self_shadow,
        normal_strength: s.view.normal_amt,
        wireframe: s.view.wireframe,
        shadows: d.shadows.enabled,
        shadow_opacity: d.shadows.opacity,
        shadow_quality: d.shadows.quality.id().into(),
        project: current.project.clone(),
    };
    if &want != current {
        settings.update(|all| all.display = want);
    }
}

/// Makes the project part of `s` the defaults for new documents.
pub fn save_project_defaults(settings: &mut UserSettings, s: &ViewState) {
    let project = project_of(s);
    settings.update(|all| all.display.project = project);
}

fn finite(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_give_the_default_view_and_round_trip() {
        let p = Prefs::default();
        let s = state_from(&p);
        assert_eq!(s, ViewState::default());
        assert_eq!(project_of(&s), p.project);
        let mut q = Prefs { tone_curve: "aces".into(), antialiasing: "high".into(), mip_bias: -2.0, lit_channels: true, ..Prefs::default() };
        q.project.projection = "orthographic".into();
        q.project.exposure_stops = 1.0;
        q.project.fov_y_degrees = 25.0;
        q.project.environment_map = "environments/sky.hdr".into();
        q.shadow_quality = "fine".into();
        let s = state_from(&q);
        assert_eq!((s.display.tone_curve, s.display.antialiasing, s.display.mip_bias, s.lit_channels), (ToneCurve::Aces, Antialiasing::High, -2.0, true));
        let back = project_of(&s);
        assert_eq!(back.projection, "orthographic");
        assert!((back.fov_y_degrees - 25.0).abs() < 1e-4 && back.exposure_stops == 1.0);
        assert_eq!((back.environment_map.as_str(), s.display.shadows.quality), ("environments/sky.hdr", ShadowQuality::Fine));
        // damaged values fall back
        let bad = Prefs { tone_curve: "sepia".into(), mip_bias: f32::NAN, ..Prefs::default() };
        let s = state_from(&bad);
        assert_eq!((s.display.tone_curve, s.display.mip_bias), (ToneCurve::Filmic, 0.0));
    }
}
