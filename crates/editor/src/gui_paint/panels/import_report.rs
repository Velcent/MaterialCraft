//! The import report: what the importer carried over exactly, approximated, baked or kept raw, with
//! a fidelity badge per entry and a filter by fidelity.

use eframe::egui::{self, Color32, RichText};
use materialcraft::paint::{Fidelity, ImportReport};

const FIDELITIES: [Fidelity; 4] = [Fidelity::Exact, Fidelity::Approximated, Fidelity::Baked, Fidelity::KeptRaw];

/// Panel state: the active filter.
#[derive(Default)]
pub struct ReportPanel {
    filter: Option<Fidelity>,
}

impl ReportPanel {
    pub fn show(&mut self, ui: &mut egui::Ui, report: Option<&ImportReport>, summary: Option<&str>) {
        crate::gui_theme::panel_header(ui, "IMPORT REPORT", false);
        let Some(report) = report else {
            ui.weak("This document was not imported.");
            return;
        };
        let source = ui.small(&report.source);
        if let Some(s) = summary.filter(|s| !s.is_empty()) {
            source.on_hover_text(format!("Importer summary: {s}"));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.filter.is_none(), format!("All {}", report.entries.len())).clicked() {
                self.filter = None;
            }
            for f in FIDELITIES {
                let n = report.count(f);
                if n > 0 && ui.selectable_label(self.filter == Some(f), badge_text(f, n)).clicked() {
                    self.filter = Some(f);
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("paint_report").auto_shrink([false, false]).show(ui, |ui| {
            for e in report.entries.iter().filter(|e| self.filter.is_none_or(|f| e.fidelity == f)) {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, e.fidelity);
                    ui.label(&e.item);
                });
                if !e.note.is_empty() {
                    ui.indent("note", |ui| ui.label(RichText::new(&e.note).small().color(Color32::from_gray(150))));
                }
            }
        });
    }
}

fn label(f: Fidelity) -> &'static str {
    match f {
        Fidelity::Exact => "exact",
        Fidelity::Approximated => "approx.",
        Fidelity::Baked => "baked",
        Fidelity::KeptRaw => "kept raw",
    }
}

fn color(f: Fidelity) -> Color32 {
    match f {
        Fidelity::Exact => Color32::from_rgb(70, 150, 90),
        Fidelity::Approximated => Color32::from_rgb(200, 150, 50),
        Fidelity::Baked => Color32::from_rgb(70, 120, 190),
        Fidelity::KeptRaw => Color32::from_rgb(170, 80, 80),
    }
}

fn badge_text(f: Fidelity, n: usize) -> RichText {
    RichText::new(format!("{} {n}", label(f))).color(color(f))
}

fn badge(ui: &mut egui::Ui, f: Fidelity) {
    let text = RichText::new(label(f)).small().strong().color(Color32::WHITE);
    egui::Frame::NONE.fill(color(f)).corner_radius(3).inner_margin(egui::Margin::symmetric(4, 0)).show(ui, |ui| ui.label(text));
}
