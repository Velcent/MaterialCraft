//! The layer panel's menus: the row context menu (LS-13, LS-17, LS-21, MK-05/06/08/09) and the add-mask
//! and add-effect menus shared with the bottom bar (MK-02, MK-14).

use super::events::{LayerEvent, SaveAsset};
use eframe::egui::{self, Ui};
use materialcraft::generators;
use materialcraft::paint::edit::ops::{MaskStart, NewEffect};
use materialcraft::paint::edit::{LayerRef, StackId};
use materialcraft::paint::{GeometryMask, Layer};

/// What the context menu acts on.
pub struct MenuTarget<'a> {
    pub layer: &'a Layer,
    pub r: LayerRef,
    /// The selection the panel operations use (contains the row's layer).
    pub ids: Vec<String>,
    pub isolated: bool,
    /// The geometry mask edit mode is on for this layer.
    pub geometry_editing: bool,
}

/// Panel actions the menu asks for that the panel itself carries out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Local {
    Rename,
}

pub fn context(ui: &mut Ui, t: &MenuTarget, events: &mut Vec<LayerEvent>) -> Option<Local> {
    let set = t.r.set.clone();
    let mut local = None;
    if ui.button("Rename").clicked() {
        local = Some(Local::Rename);
        ui.close();
    }
    if ui.button("Duplicate").clicked() {
        events.push(LayerEvent::Duplicate { set: set.clone(), ids: t.ids.clone() });
        ui.close();
    }
    if ui.button("Group into folder").clicked() {
        events.push(LayerEvent::Group { set: set.clone(), ids: t.ids.clone() });
        ui.close();
    }
    if ui.button("Save as smart material").on_hover_text("Into your asset library; a single folder is saved with its contents").clicked() {
        events.push(LayerEvent::SaveAsset(SaveAsset::SmartMaterial { set: set.clone(), ids: t.ids.clone() }));
        ui.close();
    }
    if ui.button("Delete").clicked() {
        events.push(LayerEvent::Delete { set, ids: t.ids.clone() });
        ui.close();
    }
    ui.separator();
    match &t.layer.mask {
        None => add_mask(ui, &t.r, events),
        Some(mask) => {
            ui.menu_button("Mask", |ui| {
                if ui.button(if mask.enabled { "Switch the mask off" } else { "Switch the mask on" }).clicked() {
                    events.push(LayerEvent::SetMaskEnabled(t.r.clone(), !mask.enabled));
                    ui.close();
                }
                if ui.button("Invert the starting value").on_hover_text("Black ↔ white; the mask's effects stay as they are").clicked() {
                    events.push(LayerEvent::InvertMask(t.r.clone()));
                    ui.close();
                }
                let isolate = if t.isolated { "Show the material again" } else { "Show only this mask (Alt+click)" };
                if ui.button(isolate).clicked() {
                    events.push(LayerEvent::Isolate(if t.isolated { None } else { Some(t.r.clone()) }));
                    ui.close();
                }
                add_effect(ui, "Add mask effect", &t.r, StackId::Mask, events);
                if ui.button("Save as smart mask").on_hover_text("The mask with its effects, into your asset library").clicked() {
                    events.push(LayerEvent::SaveAsset(SaveAsset::SmartMask(t.r.clone())));
                    ui.close();
                }
                ui.separator();
                add_mask_items(ui, "Replace with", &t.r, events);
                if ui.button("Remove the mask").on_hover_text("Its effects go with it; Undo brings them back").clicked() {
                    events.push(LayerEvent::RemoveMask(t.r.clone()));
                    ui.close();
                }
            });
        }
    }
    geometry_mask(ui, t, events);
    add_effect(ui, "Add effect", &t.r, StackId::Content, events);
    local
}

/// The geometry mask submenu (GM-03, GM-04): edit mode, include all, exclude all.
fn geometry_mask(ui: &mut Ui, t: &MenuTarget, events: &mut Vec<LayerEvent>) {
    ui.menu_button("Geometry mask", |ui| {
        let edit = if t.geometry_editing { "Finish editing" } else { "Choose parts…" };
        if ui.button(edit).on_hover_text("Pick parts in the viewport or from a list").clicked() {
            events.push(LayerEvent::EditGeometryMask(if t.geometry_editing { None } else { Some(t.r.clone()) }));
            ui.close();
        }
        if ui.add_enabled(t.layer.geometry_mask.is_some(), egui::Button::new("Include all")).clicked() {
            events.push(LayerEvent::SetGeometryMask(t.r.clone(), None));
            ui.close();
        }
        if ui.add_enabled(t.layer.geometry_mask != Some(GeometryMask::nothing()), egui::Button::new("Exclude all")).clicked() {
            events.push(LayerEvent::SetGeometryMask(t.r.clone(), Some(GeometryMask::nothing())));
            ui.close();
        }
    });
}

/// "Add mask" with its starting choices.
pub fn add_mask(ui: &mut Ui, r: &LayerRef, events: &mut Vec<LayerEvent>) {
    add_mask_items(ui, "Add mask", r, events);
}

fn add_mask_items(ui: &mut Ui, title: &str, r: &LayerRef, events: &mut Vec<LayerEvent>) {
    ui.menu_button(title, |ui| {
        for start in MaskStart::ALL {
            if ui.button(start.label()).clicked() {
                events.push(LayerEvent::AddMask(r.clone(), start));
                ui.close();
            }
        }
        for (id, label) in generators::MASK_STARTS {
            if ui.button(*label).clicked() {
                events.push(LayerEvent::AddMask(r.clone(), MaskStart::Generator(id)));
                ui.close();
            }
        }
    });
}

/// An add-effect submenu for one stack.
pub fn add_effect(ui: &mut Ui, title: &str, r: &LayerRef, stack: StackId, events: &mut Vec<LayerEvent>) {
    ui.menu_button(title, |ui| {
        for what in NewEffect::ALL {
            if ui.button(what.label()).clicked() {
                events.push(LayerEvent::AddEffect { layer: r.clone(), stack, what });
                ui.close();
            }
        }
        ui.weak("Generators and filters: drag them from the assets.");
    });
}
