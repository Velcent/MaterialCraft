//! The document's camera list in the camera section (CM-02, CM-05, CM-07, CM-08): pick the camera to show, rename it,
//! see whether an imported camera was changed and restore it, lock it, set it from the current view, remove it;
//! add the current view as a camera and import cameras from a glTF file.

use eframe::egui::{self, RichText, Ui};

use super::super::super::view_state::ViewState;
use super::super::PanelEvent;
use crate::gui_theme;
use materialcraft::paint::saved_view::{CameraOrigin, SavedCamera};

pub fn show(ui: &mut Ui, state: &mut ViewState, events: &mut Vec<PanelEvent>) {
    ui.add_space(4.0);
    ui.label(RichText::new("Cameras").strong());
    let list = state.camera_list();
    let active = list.active_index();
    for (i, c) in list.cameras().iter().enumerate() {
        ui.horizontal(|ui| row(ui, state, events, i, c, i == active));
    }
    ui.horizontal(|ui| {
        if ui.button("Add camera from view").on_hover_text("Keep the current view as a new camera").clicked() {
            let n = list.cameras().len();
            let index = state.add_camera_from_view(&format!("Camera {n}"));
            events.push(PanelEvent::SelectCamera(index));
        }
        if ui.button("Import cameras…").on_hover_text("Add the cameras of a .glb file (in the document mesh's units)").clicked() {
            events.push(PanelEvent::ImportCameras);
        }
    });
}

fn row(ui: &mut Ui, state: &mut ViewState, events: &mut Vec<PanelEvent>, i: usize, c: &SavedCamera, active: bool) {
    if ui.radio(active, "").on_hover_text("Show this camera").clicked() && !active {
        events.push(PanelEvent::SelectCamera(i));
    }
    name_field(ui, state, i, c);
    if c.is_modified() {
        ui.label(RichText::new("●").color(gui_theme::ACCENT)).on_hover_text("Changed since it was imported");
    }
    if i > 0 {
        let mut locked = c.locked;
        if ui.checkbox(&mut locked, "Lock").on_hover_text("A locked camera keeps its settings").changed() {
            state.lock_camera(i, locked);
        }
    }
    if c.original().is_some() {
        let can = c.is_modified() && !c.locked;
        if ui.add_enabled(can, egui::Button::new("Restore")).on_hover_text("Back to the imported values").clicked() {
            state.restore_camera(i);
        }
    }
    if i > 0 {
        let can = !active && !c.locked;
        let tip = if active { "Navigate first: the view then belongs to the default camera" } else { "Take the current view and camera settings" };
        if ui.add_enabled(can, egui::Button::new("Set from view")).on_hover_text(tip).clicked() {
            state.set_camera_from_view(i);
        }
        if ui.add_enabled(!c.locked, egui::Button::new("Remove")).on_hover_text("Remove the camera").clicked() {
            state.remove_camera(i);
        }
    }
}

/// The camera's name, editable (applied when the field loses focus).
fn name_field(ui: &mut Ui, state: &mut ViewState, i: usize, c: &SavedCamera) {
    let id = ui.id().with(("paint_camera_name", i));
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| c.name.clone());
    let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(110.0)).on_hover_text(origin_text(c));
    if response.lost_focus() {
        state.rename_camera(i, &text);
        ui.data_mut(|d| d.remove::<String>(id));
    } else if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
    }
}

fn origin_text(c: &SavedCamera) -> String {
    match &c.origin {
        CameraOrigin::Own if c.name == SavedCamera::DEFAULT_NAME => "The default camera: navigation moves it".into(),
        CameraOrigin::Own => "Made in MaterialCraft".into(),
        CameraOrigin::Imported { source, .. } => match source.as_str() {
            "gltf" => "Imported from a glTF file".into(),
            "painter-viewer" => "The view the imported project was saved with".into(),
            "painter-scene" => "A camera of the imported project's mesh".into(),
            other => format!("Imported ({other})"),
        },
    }
}
