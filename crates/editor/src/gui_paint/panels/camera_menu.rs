//! The camera dropdown of the viewport toolbar (CM-02): shown only when the document has more cameras than the
//! default one; picks the camera the view shows. Changed imported cameras carry a dot.

use eframe::egui::{self, Ui};

use super::super::view_state::ViewState;
use super::PanelEvent;

/// What the toolbar shows of the camera.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraBar {
    pub orthographic: bool,
    /// Name and "changed since import" of each camera.
    pub cameras: Vec<(String, bool)>,
    pub active: usize,
}

impl CameraBar {
    pub fn of(state: &ViewState) -> CameraBar {
        let list = state.camera_list();
        CameraBar {
            orthographic: state.display.camera.projection == materialcraft::camera::Projection::Orthographic,
            cameras: list.cameras().iter().map(|c| (c.name.clone(), c.is_modified())).collect(),
            active: list.active_index(),
        }
    }
}

/// The dropdown (nothing with a single camera); `next_key` is the shortcut that cycles cameras.
pub fn show(ui: &mut Ui, bar: &CameraBar, next_key: &str, events: &mut Vec<PanelEvent>) {
    if bar.cameras.len() < 2 {
        return;
    }
    let label = |(name, changed): &(String, bool)| if *changed { format!("{name} ●") } else { name.clone() };
    let current = bar.cameras.get(bar.active).map(label).unwrap_or_default();
    let tip = if next_key.is_empty() { "Camera".to_string() } else { format!("Camera (next: {next_key})") };
    egui::ComboBox::from_id_salt("paint_camera_menu")
        .selected_text(current)
        .width(130.0)
        .show_ui(ui, |ui| {
            for (i, c) in bar.cameras.iter().enumerate() {
                if ui.selectable_label(i == bar.active, label(c)).clicked() && i != bar.active {
                    events.push(PanelEvent::SelectCamera(i));
                }
            }
        })
        .response
        .on_hover_text(tip);
}
