//! The symmetry settings behind the quick-access bar's button (PT-37 … PT-39): mirror or radial, the object axis,
//! the mirror plane's offset or the radial copies, their point and flips, and which visual aids the view draws.
//! The plane is moved with the offset field; a handle to drag it in the view is not there yet.

use super::super::tools::helpers::{SymmetryMode, SymmetrySettings};
use eframe::egui::{self, Ui};
use materialcraft::symmetry::{Flip, MAX_RADIAL_COPIES};

const AXES: [&str; 3] = ["X", "Y", "Z"];

pub fn show(ui: &mut Ui, s: &mut SymmetrySettings) {
    ui.set_min_width(220.0);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut s.mode, SymmetryMode::Mirror, "Mirror");
        ui.selectable_value(&mut s.mode, SymmetryMode::Radial, "Radial");
    });
    ui.horizontal(|ui| {
        ui.label("Axis");
        for (i, name) in AXES.iter().enumerate() {
            ui.selectable_value(&mut s.axis, i, *name);
        }
    })
    .response
    .on_hover_text("The object axis the mirror plane lies across, or the radial copies turn around");
    match s.mode {
        SymmetryMode::Mirror => {
            ui.horizontal(|ui| {
                ui.label("Plane offset");
                ui.add(egui::DragValue::new(&mut s.offset).speed(0.005).max_decimals(4));
            })
            .response
            .on_hover_text("Where the mirror plane crosses the axis, in object units");
        }
        SymmetryMode::Radial => {
            ui.horizontal(|ui| {
                ui.label("Copies");
                ui.add(egui::DragValue::new(&mut s.count).range(2..=MAX_RADIAL_COPIES));
            });
            ui.horizontal(|ui| {
                ui.label("Through");
                for (v, name) in s.point.iter_mut().zip(AXES) {
                    ui.add(egui::DragValue::new(v).speed(0.005).max_decimals(4).prefix(format!("{name} ")));
                }
            })
            .response
            .on_hover_text("The point the axis passes through, in object units");
            ui.horizontal(|ui| {
                ui.label("Alternate copies");
                for f in Flip::ALL {
                    ui.selectable_value(&mut s.flip, f, f.label());
                }
            })
            .response
            .on_hover_text("Flip every second copy, for lettering");
        }
    }
    ui.separator();
    let a = &mut s.aids;
    ui.checkbox(&mut a.plane, if s.mode == SymmetryMode::Mirror { "Show the plane" } else { "Show the axis" });
    ui.checkbox(&mut a.meeting, "Show where it meets the mesh");
    ui.checkbox(&mut a.cursors, "Cursor at each symmetric place");
    ui.add_enabled(a.cursors, egui::Checkbox::new(&mut a.hide_cursors_while_painting, "Hide them while painting"));
}
