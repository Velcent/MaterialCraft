//! Environment and shadows (ADR 0008 mc2-vq4): which environment lights the view (the built-in studio, or a map
//! dropped from the asset tray onto the view), its rotation and alignment, exposure in stops and how it shows behind
//! the model; the studio's own light controls; scene shadows (on, opacity, how they refine) and height
//! self-shadowing.

use eframe::egui::{Color32, RichText, Ui};

use super::super::super::environment::EnvironmentStatus;
use super::super::super::view_state::ViewState;
use super::choice;
use crate::gui_theme;
use crate::gui_widgets::SliderRow;
use materialcraft::gpu3d::{EnvironmentLook, ShadowQuality, Shadows, Shared};
use materialcraft::render3d::View;

/// The colour of a map that can't be used (as the viewport's own GPU message).
const WARN: Color32 = Color32::from_rgb(255, 170, 120);

const ALIGNMENT: [(bool, &str); 2] = [(false, "Fixed to the world"), (true, "Turns with the camera")];

pub fn show(ui: &mut Ui, state: &mut ViewState, gpu: &Shared, status: &EnvironmentStatus) {
    let defaults = View::default();
    let map = state.environment_map().map(str::to_string);
    ui.horizontal(|ui| {
        ui.label("Environment:");
        match &map {
            Some(path) => {
                ui.label(RichText::new(file_name(path)).strong()).on_hover_text(path);
                if ui.button("Studio").on_hover_text("Light with the built-in studio again").clicked() {
                    state.set_environment_map(None);
                }
            }
            None => {
                ui.label(RichText::new("Built-in studio").strong());
            }
        }
    });
    let note = match status {
        EnvironmentStatus::Preparing(_) => Some(("Preparing the map…".to_string(), gui_theme::ICON_DISABLED)),
        EnvironmentStatus::Failed { error, .. } if map.is_some() => Some((format!("Can't be used ({error}); the studio lights the view."), WARN)),
        _ if map.is_none() => Some(("Drop an environment map from the asset tray onto the view to light with it.".to_string(), gui_theme::ICON_DISABLED)),
        _ => None,
    };
    if let Some((text, color)) = note {
        ui.label(RichText::new(text).small().color(color));
    }
    let studio = map.is_none();
    let view = &mut state.view;
    let mut degrees = view.light_yaw.to_degrees();
    if SliderRow::new("Rotation", &mut degrees, -180.0..=180.0).default_value(defaults.light_yaw.to_degrees()).decimals(0).suffix("°").show(ui).changed {
        view.light_yaw = degrees.to_radians();
    }
    choice(ui, "paint_env_alignment", "Alignment", &mut view.light_follows_camera, &ALIGNMENT)
        .on_hover_text("Turns with the camera: the light keeps its place on screen while you orbit");
    let look = &mut state.display.environment;
    let (lo, hi) = EnvironmentLook::EXPOSURE_RANGE;
    SliderRow::new("Exposure", &mut look.exposure_stops, lo..=hi).default_value(0.0).decimals(2).suffix(" EV").show(ui);
    if look.exposure_stops != 0.0 {
        ui.label(RichText::new("Leave the environment's exposure at 0 to compare materials consistently.").small().color(gui_theme::ICON_DISABLED));
    }
    SliderRow::new("Background opacity", &mut look.backdrop.opacity, 0.0..=1.0).default_value(0.0).show(ui);
    ui.add_enabled_ui(look.backdrop.opacity > 0.0, |ui| {
        SliderRow::new("Background blur", &mut look.backdrop.blur, 0.0..=1.0).default_value(0.3).show(ui);
    });
    if studio {
        let view = &mut state.view;
        SliderRow::new("Light elevation", &mut view.light_pitch, -1.4..=1.4).default_value(defaults.light_pitch).show(ui);
        SliderRow::new("Light intensity", &mut view.light_intensity, 0.0..=10.0).default_value(defaults.light_intensity).show(ui);
        SliderRow::new("Ambient light", &mut view.ambient, 0.0..=3.0).default_value(defaults.ambient).show(ui);
    }
    ui.separator();
    let shadows = &mut state.display.shadows;
    let shadow_defaults = Shadows::default();
    ui.checkbox(&mut shadows.enabled, "Shadows").on_hover_text("The model shadows itself under the environment's strongest light");
    ui.add_enabled_ui(shadows.enabled, |ui| {
        SliderRow::new("Shadow opacity", &mut shadows.opacity, 0.0..=1.0).default_value(shadow_defaults.opacity).show(ui);
        choice(ui, "paint_shadow_quality", "Shadow quality", &mut shadows.quality, &ShadowQuality::ALL).on_hover_text(
            "Smooth navigation: shadows that must be redrawn wait until the view rests, then soften over a moment. \
             Shadows while moving: hard shadows also while you orbit. Finest: softer, refines longer.",
        );
    });
    if let Ok(mut g) = gpu.lock() {
        ui.checkbox(&mut g.opts.self_shadow, "Height self-shadowing").on_hover_text("Contact shadows from the height map (parallax and displacement)");
    }
}

/// The last part of a library path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}
