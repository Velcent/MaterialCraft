//! "Instantiate across texture sets" (IN-07, IN-08): a checkbox per texture set; sets that lack a channel the source
//! uses are marked with a warning that their instance will be incomplete. The panel turns the choice into a
//! [`super::LayerEvent::Instantiate`].

use super::bars::channel_label;
use crate::gui_theme;
use eframe::egui::{self, Id};
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::instance::InstanceLink;
use materialcraft::paint::validate::missing_channels;
use materialcraft::paint::{Channel, Document, LayerKind};

/// One texture set the dialog offers.
#[derive(Debug, Clone, PartialEq)]
pub struct SetRow {
    pub id: String,
    pub name: String,
    /// Channels the source uses that the set does not declare.
    pub missing: Vec<Channel>,
    /// The source's own set (its instance goes directly above the source).
    pub own: bool,
}

/// The rows for instancing `source`, in document order. An instance as the source counts as its own source.
pub fn rows(doc: &Document, source: &LayerRef) -> Vec<SetRow> {
    let link = InstanceLink::new(source.set.clone(), source.layer.clone());
    let link = match doc.texture_set(&source.set).and_then(|s| s.layer(&source.layer)).map(|l| &l.kind) {
        Some(LayerKind::Instance(to)) => doc.instance_source(to).unwrap_or(link),
        _ => link,
    };
    doc.texture_sets
        .iter()
        .map(|s| SetRow { id: s.id.clone(), name: s.name.clone(), missing: missing_channels(doc, &link, s), own: s.id == source.set })
        .collect()
}

/// What the user did with the dialog this frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Open,
    Cancelled,
    /// Instantiate into these sets (ids).
    Chosen(Vec<String>),
}

pub struct InstantiateDialog {
    source: LayerRef,
    /// Set ids ticked.
    ticked: Vec<String>,
}

impl InstantiateDialog {
    /// The dialog for `source`, with no set ticked.
    pub fn new(source: LayerRef) -> InstantiateDialog {
        InstantiateDialog { source, ticked: Vec::new() }
    }

    /// The layer being instanced.
    pub fn source(&self) -> &LayerRef {
        &self.source
    }

    pub fn show(&mut self, ctx: &egui::Context, doc: &Document) -> Outcome {
        let rows = rows(doc, &self.source);
        let name = doc.texture_set(&self.source.set).and_then(|s| s.layer(&self.source.layer)).map_or("the layer", |l| l.name.as_str());
        let mut outcome = Outcome::Open;
        let modal = egui::Modal::new(Id::new("paint_instantiate_across_sets")).show(ctx, |ui| {
            ui.set_max_width(440.0);
            ui.heading("Instantiate across texture sets");
            ui.label(format!("An instance of “{name}” in each set ticked. Instances follow every change of the source; painted pixels stay with the source."));
            ui.add_space(6.0);
            for row in &rows {
                let mut on = self.ticked.contains(&row.id);
                let label = if row.own { format!("{} (the source's set)", row.name) } else { row.name.clone() };
                if ui.checkbox(&mut on, label).changed() {
                    self.ticked.retain(|id| *id != row.id);
                    if on {
                        self.ticked.push(row.id.clone());
                    }
                }
                if !row.missing.is_empty() {
                    let names: Vec<String> = row.missing.iter().map(|c| channel_label(*c)).collect();
                    ui.indent(("missing", &row.id), |ui| {
                        ui.colored_label(gui_theme::ACCENT, format!("⚠ Lacks {}: the instance will be incomplete.", names.join(", ")));
                    });
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.small_button("All").clicked() {
                    self.ticked = rows.iter().map(|r| r.id.clone()).collect();
                }
                if ui.small_button("None").clicked() {
                    self.ticked.clear();
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.ticked.is_empty(), egui::Button::new("Instantiate")).clicked() {
                    // in document order
                    outcome = Outcome::Chosen(rows.iter().filter(|r| self.ticked.contains(&r.id)).map(|r| r.id.clone()).collect());
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancelled;
                }
            });
        });
        if outcome == Outcome::Open && modal.should_close() {
            outcome = Outcome::Cancelled;
        }
        outcome
    }
}
