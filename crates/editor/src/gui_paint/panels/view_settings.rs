//! Display settings: how height is shown, exposure and the light. Edits the viewport's `View`
//! directly (it is pure display state, nothing in the document changes); every number is a slider row.

use eframe::egui;
use materialcraft::gpu3d::Shared;
use materialcraft::render3d::{RELIEF_BUMP, RELIEF_DISPLACE, RELIEF_NONE, RELIEF_PARALLAX, View};

use crate::gui_widgets::SliderRow;

const RELIEF: [(u32, &str); 4] =
    [(RELIEF_NONE, "Normal map only"), (RELIEF_BUMP, "Bump"), (RELIEF_PARALLAX, "Parallax (outward)"), (RELIEF_DISPLACE, "Displacement")];

pub fn show(ui: &mut egui::Ui, view: &mut View, gpu: &Shared) {
    let defaults = View::default();
    ui.strong("Height");
    egui::ComboBox::from_id_salt("paint_relief").width(170.0).selected_text(RELIEF.iter().find(|r| r.0 == view.relief).map_or("?", |r| r.1)).show_ui(
        ui,
        |ui| {
            for (id, label) in RELIEF {
                ui.selectable_value(&mut view.relief, id, label);
            }
        },
    );
    if view.relief == RELIEF_BUMP {
        SliderRow::new("Bump strength", &mut view.bump, 0.0..=12.0).default_value(defaults.bump).show(ui);
    }
    if view.relief >= RELIEF_PARALLAX {
        SliderRow::new("Height scale", &mut view.height_scale, 0.0..=0.5).default_value(defaults.height_scale).show(ui);
        SliderRow::new("Neutral level", &mut view.height_level, 0.0..=1.0).default_value(defaults.height_level).show(ui);
        if let Ok(mut g) = gpu.lock() {
            ui.checkbox(&mut g.opts.self_shadow, "Height self-shadowing");
        }
    }
    SliderRow::new("Normal strength", &mut view.normal_amt, 0.0..=1.0).default_value(defaults.normal_amt).show(ui);
    ui.add_space(6.0);
    ui.strong("Camera");
    SliderRow::new("Exposure", &mut view.exposure, 0.1..=4.0).default_value(defaults.exposure).show(ui);
    SliderRow::new("Ambient light", &mut view.ambient, 0.0..=3.0).default_value(defaults.ambient).show(ui);
    ui.add_space(6.0);
    ui.strong("Light");
    ui.checkbox(&mut view.light_follows_camera, "Follows the camera");
    SliderRow::new("Intensity", &mut view.light_intensity, 0.0..=10.0).default_value(defaults.light_intensity).show(ui);
    SliderRow::new("Azimuth", &mut view.light_yaw, -std::f32::consts::PI..=std::f32::consts::PI).default_value(defaults.light_yaw).show(ui);
    SliderRow::new("Elevation", &mut view.light_pitch, -1.4..=1.4).default_value(defaults.light_pitch).show(ui);
}
