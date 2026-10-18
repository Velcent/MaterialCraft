//! The geometry mask window shown during the edit mode: the set's parts with a tick each (GM-03), include all,
//! exclude all (GM-04) and Done.

use super::MaskTarget;
use crate::gui_paint::panels::layers::LayerEvent;
use eframe::egui::{self, Context, ScrollArea};
use materialcraft::paint::GeometryMask;

/// Draws the window for `target`; the changes asked for go to `out` (leaving the mode as `EditGeometryMask(None)`).
pub fn show(ctx: &Context, target: &MaskTarget, out: &mut Vec<LayerEvent>) {
    let mut open = true;
    egui::Window::new("Geometry mask").id(egui::Id::new("geometry_mask_edit")).open(&mut open).collapsible(false).resizable(true).default_width(240.0).show(
        ctx,
        |ui| {
            ui.label(format!("{} / {}", target.set().name, target.layer().name));
            ui.weak("Click a part in the viewport to exclude it; excluded parts are hidden until you finish. Tick a part here to bring it back.");
            ui.separator();
            let parts = target.parts();
            if parts.is_empty() {
                ui.weak("The mesh has no parts for this texture set.");
            }
            ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for name in &parts {
                    let mut on = target.includes(name);
                    if ui.checkbox(&mut on, name.as_str()).changed() {
                        out.extend(target.set_part(name, on));
                    }
                }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Include all").clicked() {
                    out.extend(target.replace(None));
                }
                if ui.button("Exclude all").clicked() {
                    out.extend(target.replace(Some(GeometryMask::nothing())));
                }
                if ui.button("Done").clicked() {
                    out.push(LayerEvent::EditGeometryMask(None));
                }
            });
        },
    );
    if !open {
        out.push(LayerEvent::EditGeometryMask(None));
    }
}
