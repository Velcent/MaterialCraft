//! Post effects (ADR 0008 mc2-vq5): the tone curve, accumulated anti-aliasing and the depth-of-field switch.
//! Bloom and colour grading come later.

use eframe::egui::Ui;

use super::super::super::view_state::ViewState;
use super::choice;
use materialcraft::camera::Projection;
use materialcraft::gpu3d::{Antialiasing, ToneCurve};

pub fn show(ui: &mut Ui, state: &mut ViewState) {
    let d = &mut state.display;
    choice(ui, "paint_tone_curve", "Tone curve", &mut d.tone_curve, &ToneCurve::ALL);
    choice(ui, "paint_antialiasing", "Anti-aliasing", &mut d.antialiasing, &Antialiasing::ALL)
        .on_hover_text("Smooths edges and fine detail over several frames while the view stands still");
    let perspective = d.camera.projection == Projection::Perspective;
    ui.add_enabled(perspective, eframe::egui::Checkbox::new(&mut d.camera.depth_of_field.enabled, "Depth of field"))
        .on_hover_text("Blurs what is nearer or farther than the focus distance (Camera section) while the view stands still")
        .on_disabled_hover_text("Needs a perspective camera");
}
