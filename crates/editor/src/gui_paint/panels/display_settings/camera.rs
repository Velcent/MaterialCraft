//! Camera: projection, field of view and focal length (linked through one sensor size), exposure in stops, and
//! the depth of field's focus distance and aperture (active only while depth of field is on, CM-09).

use eframe::egui::Ui;

use super::super::super::view_state::ViewState;
use super::super::PanelEvent;
use super::choice;
use crate::gui_widgets::SliderRow;
use materialcraft::camera::{CameraSettings, DepthOfField, Orbit, Projection, fov_y};
use materialcraft::gpu3d::Display;

const PROJECTIONS: [(Projection, &str); 2] = [(Projection::Perspective, "Perspective"), (Projection::Orthographic, "Orthographic")];

pub fn show(ui: &mut Ui, state: &mut ViewState, events: &mut Vec<PanelEvent>) {
    ui.label(format!("Camera: {}", state.camera_name()));
    let d = &mut state.display;
    let defaults = CameraSettings::default();
    choice(ui, "paint_projection", "Projection", &mut d.camera.projection, &PROJECTIONS);
    let mut fov = d.camera.fov_y.to_degrees();
    let (lo, hi) = (CameraSettings::FOV_RANGE.0.to_degrees(), CameraSettings::FOV_RANGE.1.to_degrees());
    if SliderRow::new("Field of view", &mut fov, lo..=hi).default_value(defaults.fov_y.to_degrees()).decimals(1).suffix("°").show(ui).changed {
        d.camera.set_fov_y(fov.to_radians());
    }
    let mut mm = d.camera.focal_length();
    let (mm_lo, mm_hi) = (materialcraft::camera::focal_length(CameraSettings::FOV_RANGE.1), materialcraft::camera::focal_length(CameraSettings::FOV_RANGE.0));
    if SliderRow::new("Focal length", &mut mm, mm_lo..=mm_hi).default_value(defaults.focal_length()).decimals(1).suffix(" mm").show(ui).changed {
        d.camera.set_fov_y(fov_y(mm));
    }
    let (elo, ehi) = Display::EXPOSURE_RANGE;
    SliderRow::new("Exposure", &mut d.exposure_stops, elo..=ehi).default_value(0.0).decimals(2).suffix(" EV").show(ui);
    let dof_on = d.camera.depth_of_field.enabled;
    let dof_default = DepthOfField::default();
    ui.add_enabled_ui(dof_on, |ui| {
        let dof = &mut d.camera.depth_of_field;
        let (near, far) = Orbit::DISTANCE;
        SliderRow::new("Focus distance", &mut dof.focus_distance, 0.1..=far * 2.0).default_value(dof_default.focus_distance).show(ui);
        SliderRow::new("Aperture", &mut dof.aperture, 0.0..=0.3).default_value(dof_default.aperture).decimals(3).show(ui);
        if ui.button("Focus on the pivot").on_hover_text("Sharp at the point the camera orbits").clicked() {
            dof.focus_distance = state.view.dist.max(near * 0.5);
        }
    })
    .response
    .on_disabled_hover_text("Turn on depth of field under Post effects");
    ui.horizontal(|ui| {
        if ui.button("Reset camera").clicked() {
            events.push(PanelEvent::ResetCamera);
        }
        if ui.button("Use as defaults").on_hover_text("New and imported documents start with this camera, exposure and environment").clicked() {
            events.push(PanelEvent::UseDisplayDefaults);
        }
    });
}
