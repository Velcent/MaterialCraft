//! The status bar: the last message, load or import progress (with Cancel) and map progress.

use super::PanelEvent;
use eframe::egui::{self, Color32};
use materialcraft::diag::{Entry, Level};

/// Shown before anything was reported.
const HINT: &str = "Open a .mcpaint document or import a Substance Painter project (.spp).";

/// What is running in the background.
pub struct Activity<'a> {
    /// (fraction, step) of a load or import.
    pub loading: Option<(f32, &'a str)>,
    /// (texture sets done, total) of the map computation.
    pub maps: Option<(usize, usize)>,
}

/// Draws the bar; `status` is the newest diagnostics entry of the session.
pub fn show(ui: &mut egui::Ui, status: Option<&Entry>, activity: &Activity, events: &mut Vec<PanelEvent>) {
    let frame = egui::Frame::NONE.fill(crate::gui_theme::HEADER_BG).inner_margin(egui::Margin::symmetric(8, 2));
    egui::Panel::bottom("paint_status").frame(frame).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (text, col) = match status {
                Some(e) => (e.text.as_str(), color(e.level)),
                None => (HINT, color(Level::Info)),
            };
            ui.colored_label(col, text);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some((f, step)) = activity.loading {
                    if ui.small_button("Cancel").clicked() {
                        events.push(PanelEvent::CancelLoad);
                    }
                    ui.add(egui::ProgressBar::new(f).desired_width(160.0).show_percentage());
                    ui.label(step);
                } else if let Some((done, total)) = activity.maps {
                    ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).desired_width(120.0));
                    ui.label(format!("Compositing texture sets {done}/{total}"));
                }
            });
        });
    });
}

/// The look of each level (LG-02), shared with the log panel.
pub fn color(level: Level) -> Color32 {
    match level {
        Level::Debug | Level::Info => Color32::from_gray(170),
        Level::Warning => Color32::from_rgb(230, 180, 90),
        Level::Error => Color32::from_rgb(255, 120, 110),
    }
}
