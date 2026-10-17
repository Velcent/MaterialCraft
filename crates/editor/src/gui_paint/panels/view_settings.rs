//! View settings: how height is shown, exposure and the light. Edits the viewport's `View` directly
//! (it is pure display state, nothing in the document changes).

use eframe::egui::{self, Slider};
use materialcraft::gpu3d::Shared;
use materialcraft::render3d::{RELIEF_BUMP, RELIEF_DISPLACE, RELIEF_NONE, RELIEF_PARALLAX, View};

const RELIEF: [(u32, &str); 4] =
    [(RELIEF_NONE, "Normal map only"), (RELIEF_BUMP, "Bump"), (RELIEF_PARALLAX, "Parallax (outward)"), (RELIEF_DISPLACE, "Displacement")];

pub fn show(ui: &mut egui::Ui, view: &mut View, gpu: &Shared) {
    crate::gui_theme::panel_header(ui, "VIEW SETTINGS", false);
    ui.add_space(4.0);
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
        ui.add(Slider::new(&mut view.bump, 0.0..=12.0).text("Bump strength"));
    }
    if view.relief >= RELIEF_PARALLAX {
        ui.add(Slider::new(&mut view.height_scale, 0.0..=0.5).text("Height scale"));
        ui.add(Slider::new(&mut view.height_level, 0.0..=1.0).text("Neutral level"));
        if let Ok(mut g) = gpu.lock() {
            ui.checkbox(&mut g.opts.self_shadow, "Height self-shadowing");
        }
    }
    ui.add(Slider::new(&mut view.normal_amt, 0.0..=1.0).text("Normal strength"));
    ui.add_space(6.0);
    ui.strong("Camera");
    ui.add(Slider::new(&mut view.exposure, 0.1..=4.0).text("Exposure"));
    ui.add(Slider::new(&mut view.ambient, 0.0..=3.0).text("Ambient light"));
    ui.add_space(6.0);
    ui.strong("Light");
    ui.checkbox(&mut view.light_follows_camera, "Follows the camera");
    ui.add(Slider::new(&mut view.light_intensity, 0.0..=10.0).text("Intensity"));
    ui.add(Slider::new(&mut view.light_yaw, -std::f32::consts::PI..=std::f32::consts::PI).text("Azimuth"));
    ui.add(Slider::new(&mut view.light_pitch, -1.4..=1.4).text("Elevation"));
}
