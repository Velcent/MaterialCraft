//! The row of display modes above the viewport: Material or a single channel.

use super::super::channel_view::ChannelView;
use super::PanelEvent;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, current: ChannelView, events: &mut Vec<PanelEvent>) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label("View");
        for (view, label) in ChannelView::ALL {
            if ui.selectable_label(view == current, label).clicked() && view != current {
                events.push(PanelEvent::ShowChannel(view));
            }
        }
    });
}
