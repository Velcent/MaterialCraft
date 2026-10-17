//! The polygon fill tool's quick-access controls (PT-45, PT-46): what a click grows to, the drag shape, whether a
//! drag picks through the model, and white (fill) or black (un-pick).

use super::super::tools::polygon_fill::{DragShape, PolygonFillSettings};
use eframe::egui::{self, Ui};
use materialcraft::paint::effect::PickMode;

fn mode_label(m: PickMode) -> &'static str {
    match m {
        PickMode::Triangle => "Triangle",
        PickMode::Polygon => "Face",
        PickMode::Mesh => "Mesh piece",
        PickMode::UvIsland => "UV island",
    }
}

pub fn show(ui: &mut Ui, s: &mut PolygonFillSettings) {
    egui::ComboBox::from_id_salt("polygon_fill_mode").selected_text(mode_label(s.mode)).show_ui(ui, |ui| {
        for m in PickMode::ALL {
            ui.selectable_value(&mut s.mode, m, mode_label(m));
        }
    });
    ui.selectable_value(&mut s.shape, DragShape::Rectangle, "Rectangle").on_hover_text("A drag picks what the rectangle touches");
    ui.selectable_value(&mut s.shape, DragShape::Lasso, "Lasso").on_hover_text("A drag picks what the lasso touches");
    ui.checkbox(&mut s.visible_only, "Visible only").on_hover_text("A drag picks only what the camera sees, not through the model");
    ui.separator();
    let label = if s.white { "Fill (white)" } else { "Un-pick (black)" };
    if ui.button(label).on_hover_text("White fills the picks, black takes them out again").clicked() {
        s.white = !s.white;
    }
}
