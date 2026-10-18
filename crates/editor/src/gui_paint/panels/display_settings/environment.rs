//! Environment and shadows: the studio environment's key light and ambient light, height self-shadowing.

use eframe::egui::Ui;
use std::f32::consts::PI;

use super::super::super::view_state::ViewState;
use crate::gui_widgets::SliderRow;
use materialcraft::gpu3d::Shared;
use materialcraft::render3d::View;

pub fn show(ui: &mut Ui, state: &mut ViewState, gpu: &Shared) {
    let defaults = View::default();
    let view = &mut state.view;
    ui.label("Environment: built-in studio");
    SliderRow::new("Ambient light", &mut view.ambient, 0.0..=3.0).default_value(defaults.ambient).show(ui);
    ui.checkbox(&mut view.light_follows_camera, "Light follows the camera");
    SliderRow::new("Light intensity", &mut view.light_intensity, 0.0..=10.0).default_value(defaults.light_intensity).show(ui);
    SliderRow::new("Light azimuth", &mut view.light_yaw, -PI..=PI).default_value(defaults.light_yaw).show(ui);
    SliderRow::new("Light elevation", &mut view.light_pitch, -1.4..=1.4).default_value(defaults.light_pitch).show(ui);
    if let Ok(mut g) = gpu.lock() {
        ui.checkbox(&mut g.opts.self_shadow, "Height self-shadowing").on_hover_text("Contact shadows from the height map (parallax and displacement)");
    }
}
