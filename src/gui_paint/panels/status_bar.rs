//! The status bar: the last message, load or import progress (with Cancel) and map progress.

use super::PanelEvent;
use eframe::egui::{self, Color32};

/// The last message for the user.
pub struct Status {
    pub text: String,
    pub ok: bool,
}

impl Status {
    pub fn info(text: impl Into<String>) -> Status {
        Status {
            text: text.into(),
            ok: true,
        }
    }

    pub fn error(text: impl Into<String>) -> Status {
        Status {
            text: text.into(),
            ok: false,
        }
    }
}

/// What is running in the background.
pub struct Activity<'a> {
    /// (fraction, step) of a load or import.
    pub loading: Option<(f32, &'a str)>,
    /// (texture sets done, total) of the map computation.
    pub maps: Option<(usize, usize)>,
}

pub fn show(
    ctx: &egui::Context,
    status: &Status,
    activity: &Activity,
    events: &mut Vec<PanelEvent>,
) {
    let frame = egui::Frame::none()
        .fill(Color32::from_rgb(30, 30, 30))
        .inner_margin(egui::Margin::symmetric(8.0, 2.0));
    egui::TopBottomPanel::bottom("paint_status")
        .frame(frame)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let col = if status.ok {
                    Color32::from_gray(170)
                } else {
                    Color32::from_rgb(255, 120, 110)
                };
                ui.colored_label(col, &status.text);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some((f, step)) = activity.loading {
                        if ui.small_button("Cancel").clicked() {
                            events.push(PanelEvent::CancelLoad);
                        }
                        ui.add(
                            egui::ProgressBar::new(f)
                                .desired_width(160.0)
                                .show_percentage(),
                        );
                        ui.label(step);
                    } else if let Some((done, total)) = activity.maps {
                        ui.add(
                            egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                                .desired_width(120.0),
                        );
                        ui.label(format!("Compositing texture sets {done}/{total}"));
                    }
                });
            });
        });
}
