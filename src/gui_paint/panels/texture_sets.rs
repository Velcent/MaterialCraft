//! The texture set list: name, resolution, channels, whether its maps are ready, and visibility.

use super::super::document_state::OpenDocument;
use super::PanelEvent;
use eframe::egui::{self, Color32};

pub fn show(ui: &mut egui::Ui, doc: &OpenDocument, events: &mut Vec<PanelEvent>) {
    crate::panel_header(ui, &format!("TEXTURE SETS ({})", doc.sets.len()), false);
    if doc.doc().texture_sets.is_empty() {
        ui.weak("This document has no texture sets.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("paint_sets")
        .max_height(ui.available_height() * 0.45)
        .show(ui, |ui| {
            for (i, (set, state)) in doc.doc().texture_sets.iter().zip(&doc.sets).enumerate() {
                ui.horizontal(|ui| {
                    let mut visible = state.visible;
                    if ui
                        .checkbox(&mut visible, "")
                        .on_hover_text("Show this texture set")
                        .changed()
                    {
                        events.push(PanelEvent::SetVisible(i, visible));
                    }
                    let name = egui::RichText::new(&set.name).color(if visible {
                        Color32::from_gray(225)
                    } else {
                        Color32::from_gray(120)
                    });
                    ui.label(name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if visible && !state.maps_ready {
                            ui.spinner();
                        }
                        ui.small(format!("{}x{}", set.resolution.0, set.resolution.1));
                    });
                })
                .response
                .on_hover_text(details(set));
            }
        });
}

fn details(set: &materialcraft::paint::TextureSet) -> String {
    let channels: Vec<String> = set.channels.iter().map(|c| c.channel.id()).collect();
    format!(
        "{} layers (top level), {} baked maps\nChannels: {}",
        set.layers.len(),
        set.baked.len(),
        channels.join(", ")
    )
}
