//! The view mode in the viewport toolbar: Material or a single channel.

use super::super::channel_view::ChannelView;
use super::PanelEvent;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, current: ChannelView, events: &mut Vec<PanelEvent>) {
    ui.label("View");
    let name = ChannelView::ALL.iter().find(|(v, _)| *v == current).map_or("?", |(_, l)| *l);
    egui::ComboBox::from_id_salt("paint_view_mode").selected_text(name).width(120.0).show_ui(ui, |ui| {
        for (view, label) in ChannelView::ALL {
            if ui.selectable_label(view == current, label).clicked() && view != current {
                events.push(PanelEvent::ShowChannel(view));
            }
        }
    });
}
