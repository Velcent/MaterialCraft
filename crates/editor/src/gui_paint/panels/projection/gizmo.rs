//! The view gizmo's settings (PJ-23): shown or not, move/rotate/scale, local or world axes, the step sizes of
//! stepped drags (Ctrl) and the gizmo's size on screen.

use crate::gui_paint::projection_edit::GizmoSettings;
use eframe::egui::{DragValue, Ui};
use materialcraft::projection::gizmo::{GizmoMode, Space};

pub fn show(ui: &mut Ui, s: &mut GizmoSettings) {
    ui.checkbox(&mut s.shown, "Show the gizmo in the view");
    ui.add_enabled_ui(s.shown, |ui| {
        ui.horizontal(|ui| {
            for m in GizmoMode::ALL {
                ui.selectable_value(&mut s.mode, m, m.label());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Axes");
            ui.selectable_value(&mut s.space, Space::Local, "Local").on_hover_text("The volume's own axes");
            ui.selectable_value(&mut s.space, Space::World, "World").on_hover_text("The object's axes (scaling always follows the volume's)");
        });
        ui.horizontal(|ui| {
            ui.label("Steps (Ctrl)");
            ui.add(DragValue::new(&mut s.steps.distance).speed(0.005).range(0.001..=10.0).prefix("move ").max_decimals(3))
                .on_hover_text("In scene units: 1 is the scene's largest extent");
            ui.add(DragValue::new(&mut s.steps.angle).speed(0.5).range(1.0..=180.0).suffix("°"));
            ui.add(DragValue::new(&mut s.steps.scale).speed(0.005).range(0.001..=10.0).prefix("scale ").max_decimals(3));
        });
        ui.horizontal(|ui| {
            ui.label("Size");
            ui.add(DragValue::new(&mut s.size).speed(1.0).range(30.0..=400.0).suffix(" pt"));
        });
        ui.weak(
            "Drag the arrows, rings or knobs; the centre moves in the view plane (Shift: onto the surface, planar and warp). \
             UV mode: drag inside the frame to move (Shift: one axis), the knobs to scale, outside to turn. \
             Warp: with the grid points shown, drag a point.",
        );
    });
}
