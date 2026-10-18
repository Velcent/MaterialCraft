//! The display settings panel (DS-01, ADR 0008 mc2-vq4/vq7): how the viewport shows the document, in four
//! sections: environment and shadows, camera, post effects and viewport. It edits the viewport's
//! [`ViewState`] and the renderer's options directly: pure display state, nothing in the document changes
//! (DS-14). Every change shows at once (DS-02); the document keeps its camera, exposure and environment when saved,
//! everything else is remembered as a user default.

mod camera;
mod camera_list;
mod environment;
mod post;
mod viewport;

use eframe::egui::{self, Ui};

use super::super::environment::EnvironmentStatus;
use super::super::view_state::ViewState;
use super::PanelEvent;
use materialcraft::gpu3d::Shared;

pub fn show(ui: &mut Ui, state: &mut ViewState, gpu: &Shared, environment: &EnvironmentStatus, events: &mut Vec<PanelEvent>) {
    section(ui, "Environment and shadows", |ui| environment::show(ui, state, gpu, environment));
    section(ui, "Camera", |ui| camera::show(ui, state, events));
    section(ui, "Post effects", |ui| post::show(ui, state));
    section(ui, "Viewport", |ui| viewport::show(ui, state, gpu));
}

fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(egui::RichText::new(title).strong()).id_salt(("paint_display", title)).default_open(true).show(ui, body);
}

/// A combo box over `(value, label)` choices.
fn choice<T: Copy + PartialEq>(ui: &mut Ui, id: &str, label: &str, value: &mut T, choices: &[(T, &str)]) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(label);
        let current = choices.iter().find(|(v, _)| v == value).map_or("?", |(_, l)| *l);
        egui::ComboBox::from_id_salt(id).width(150.0).selected_text(current).show_ui(ui, |ui| {
            for (v, l) in choices {
                ui.selectable_value(value, *v, *l);
            }
        });
    })
    .response
}
