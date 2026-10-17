//! The bakers panel (roadmap step 28, BK2-01…BK2-09): the selected texture set's bake configuration, its maps
//! with their toggles and settings, link, copy and check commands, and the bake buttons with progress and cancel.
//! "Bake mode" is the built-in Baking layout with this panel docked (ADR 0008 mc2-vq18).

mod common;
mod events;
mod fields;
mod maps;

pub use events::BakerEvent;

use super::PanelEvent;
use crate::gui_paint::document_state::OpenDocument;
use crate::gui_theme;
use eframe::egui::{self, RichText, Ui};
use fields::Fields;
use maps::MapRows;
use materialcraft::bake::MapKind;
use materialcraft::history::CoalesceKey;
use materialcraft::paint::bake_config::BakePart;
use std::time::{Duration, Instant};

/// How long the setting a log entry led to stays outlined.
const HIGHLIGHT: Duration = Duration::from_millis(1600);

/// The panel's view state.
#[derive(Default)]
pub struct BakersPanel {
    /// The texture set shown, by index.
    set: usize,
    /// The map whose settings are open.
    open: Option<MapKind>,
    drag: Option<CoalesceKey>,
    next_key: u64,
    /// The field a log entry led to, since when, and whether it was scrolled to.
    highlight: Option<(String, Instant, bool)>,
}

/// The running bake, for the panel's progress line.
pub struct BakeStatus<'a> {
    /// (set name, done, total)
    pub running: Option<(&'a str, usize, usize)>,
}

impl BakersPanel {
    /// Shows `field` (`<section>.<field>`) of the set at `set`: selects the set, opens the map, outlines the row.
    pub fn reveal(&mut self, set: usize, field: &str) {
        self.set = set;
        let section = field.split('.').next().unwrap_or_default();
        if let Some(kind) = MapKind::from_name(section) {
            self.open = Some(kind);
        }
        self.highlight = Some((field.to_string(), Instant::now(), false));
    }

    pub fn show(&mut self, ui: &mut Ui, doc: &OpenDocument, status: &BakeStatus, out: &mut Vec<PanelEvent>) {
        let d = doc.doc();
        if d.texture_sets.is_empty() {
            ui.weak("This document has no texture sets.");
            return;
        }
        self.set = self.set.min(d.texture_sets.len() - 1);
        let Some(set) = d.texture_sets.get(self.set) else { return };
        let all: Vec<String> = d.texture_sets.iter().map(|s| s.id.clone()).collect();
        let several = all.len() > 1;
        let mut events = Vec::new();
        self.header(ui, doc, status, &all, &mut events);
        ui.separator();
        let highlight = match &mut self.highlight {
            Some((_, since, _)) if since.elapsed() > HIGHLIGHT => {
                self.highlight = None;
                None
            }
            Some((field, _, scrolled)) => {
                ui.ctx().request_repaint_after(Duration::from_millis(100));
                Some((field.clone(), !std::mem::replace(scrolled, true)))
            }
            None => None,
        };
        let mut open = self.open;
        egui::ScrollArea::vertical().id_salt("paint_bakers").show(ui, |ui| {
            let mut f = Fields {
                highlight: highlight.as_ref().map(|(k, s)| (k.as_str(), *s)),
                drag: &mut self.drag,
                next_key: &mut self.next_key,
                events: &mut events,
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new("Common").strong().color(gui_theme::HEADER_TEXT));
                if several {
                    let linked = d.bake_links.common;
                    let r = ui.selectable_label(linked, "Link").on_hover_text("Linked: a change of the common settings is made in every texture set");
                    if r.clicked() {
                        f.events.push(BakerEvent::Link { set: set.id.clone(), part: BakePart::Common, on: !linked });
                    }
                }
            });
            common::show(ui, &set.id, &set.bake.common, &mut f);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Maps").strong().color(gui_theme::HEADER_TEXT));
                checks_menu(ui, &set.id, &all, &mut f);
            });
            let rows = MapRows { set: &set.id, links: d.bake_links, several_sets: several, all_sets: &all, busy: status.running.is_some() };
            for entry in set.bake.maps.iter() {
                let kind = entry.kind();
                if rows.row(ui, entry, open == Some(kind), &mut f) {
                    open = if open == Some(kind) { None } else { Some(kind) };
                }
                if open == Some(kind) {
                    ui.indent(("bakers_settings", kind.name()), |ui| rows.settings(ui, entry, &mut f));
                }
            }
        });
        self.open = open;
        out.extend(events.into_iter().map(PanelEvent::Bakers));
    }

    /// Set choice, bake buttons, progress and cancel.
    fn header(&mut self, ui: &mut Ui, doc: &OpenDocument, status: &BakeStatus, all: &[String], events: &mut Vec<BakerEvent>) {
        let d = doc.doc();
        ui.horizontal(|ui| {
            ui.label("Texture set");
            let name = d.texture_sets.get(self.set).map_or("", |s| s.name.as_str());
            egui::ComboBox::from_id_salt("bakers_set").selected_text(name).show_ui(ui, |ui| {
                for (i, s) in d.texture_sets.iter().enumerate() {
                    ui.selectable_value(&mut self.set, i, &s.name);
                }
            });
        });
        let Some(set) = d.texture_sets.get(self.set) else { return };
        ui.horizontal_wrapped(|ui| match status.running {
            Some((name, done, total)) => {
                if ui.button("Cancel").clicked() {
                    events.push(BakerEvent::Cancel);
                }
                ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).desired_width(90.0));
                ui.label(format!("Baking {name} ({}/{total})", done + 1));
            }
            None => {
                if ui.button("Bake this set").clicked() {
                    events.push(BakerEvent::Bake { sets: vec![set.id.clone()], maps: None });
                }
                if all.len() > 1 && ui.button("Bake all sets").on_hover_text("One set after another").clicked() {
                    events.push(BakerEvent::Bake { sets: all.to_vec(), maps: None });
                }
                if all.len() > 1 {
                    ui.menu_button("Copy…", |ui| {
                        if ui.button("Copy all settings to every set").clicked() {
                            events.push(BakerEvent::CopyAll { from: set.id.clone(), to: all.to_vec() });
                            ui.close();
                        }
                        if ui.button("Copy the common settings to every set").clicked() {
                            events.push(BakerEvent::CopyPart { from: set.id.clone(), part: BakePart::Common, to: all.to_vec() });
                            ui.close();
                        }
                        ui.separator();
                        for other in d.texture_sets.iter().filter(|s| s.id != set.id) {
                            if ui.button(format!("Copy all settings to {}", other.name)).clicked() {
                                events.push(BakerEvent::CopyAll { from: set.id.clone(), to: vec![other.id.clone()] });
                                ui.close();
                            }
                        }
                    });
                }
            }
        });
    }
}

/// Check all, none, invert, copy the checks (BK2-05).
fn checks_menu(ui: &mut Ui, set: &str, all: &[String], f: &mut Fields) {
    ui.menu_button("Checks…", |ui| {
        let mut picked = None;
        if ui.button("Check all").clicked() {
            picked = Some(BakerEvent::CheckAll { set: set.to_string(), on: true });
        }
        if ui.button("Uncheck all").clicked() {
            picked = Some(BakerEvent::CheckAll { set: set.to_string(), on: false });
        }
        if ui.button("Invert").clicked() {
            picked = Some(BakerEvent::InvertChecks { set: set.to_string() });
        }
        if all.len() > 1 && ui.button("Copy the checks to every set").clicked() {
            picked = Some(BakerEvent::CopyChecks { from: set.to_string(), to: all.to_vec() });
        }
        if let Some(ev) = picked {
            f.events.push(ev);
            ui.close();
        }
    });
}
