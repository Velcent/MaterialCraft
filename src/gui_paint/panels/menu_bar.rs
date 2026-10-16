//! The Paint screen's menu bar.

use super::PanelEvent;
use eframe::egui::{self, Color32};

/// Draws the menu bar; `has_document` enables the document entries.
pub fn show(ctx: &egui::Context, title: &str, has_document: bool, events: &mut Vec<PanelEvent>) {
    let frame = egui::Frame::none()
        .fill(Color32::from_rgb(34, 34, 34))
        .inner_margin(egui::Margin::symmetric(6.0, 2.0));
    egui::TopBottomPanel::top("paint_menu")
        .frame(frame)
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    let mut item =
                        |ui: &mut egui::Ui, label: &str, enabled: bool, ev: PanelEvent| {
                            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                                events.push(ev);
                                ui.close_menu();
                            }
                        };
                    item(ui, "Open in Paint mode...", true, PanelEvent::Open);
                    item(
                        ui,
                        "Import Substance Painter project...",
                        true,
                        PanelEvent::Import,
                    );
                    item(ui, "Close document", has_document, PanelEvent::Close);
                    ui.separator();
                    item(ui, "Home screen", true, PanelEvent::Home);
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .add_enabled(has_document, egui::Button::new("Reset camera"))
                        .clicked()
                    {
                        events.push(PanelEvent::ResetCamera);
                        ui.close_menu();
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(title).color(Color32::from_gray(170)));
                    ui.label(
                        egui::RichText::new("PAINT")
                            .strong()
                            .color(Color32::from_rgb(235, 130, 20)),
                    );
                });
            });
        });
}
