//! The view mode dropdown (DS-03): the material, a single channel or a baked mesh map. The same control sits in the
//! viewport toolbar, the 2D view's corner and the display settings.

use super::super::channel_view::ChannelView;
use super::PanelEvent;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, current: ChannelView, events: &mut Vec<PanelEvent>) {
    show_with_id(ui, "paint_view_mode", current, events);
}

/// The dropdown with its own id (several are shown at once).
pub fn show_with_id(ui: &mut egui::Ui, id: &str, current: ChannelView, events: &mut Vec<PanelEvent>) {
    ui.label("View");
    egui::ComboBox::from_id_salt(id).selected_text(current.label()).width(140.0).show_ui(ui, |ui| {
        for (view, label) in ChannelView::ALL {
            if ui.selectable_label(view == current, label).clicked() && view != current {
                events.push(PanelEvent::ShowChannel(view));
            }
        }
        ui.separator();
        ui.weak("Mesh maps");
        for (view, label) in ChannelView::MESH_MAPS {
            if ui.selectable_label(view == current, label).clicked() && view != current {
                events.push(PanelEvent::ShowChannel(view));
            }
        }
    });
}
