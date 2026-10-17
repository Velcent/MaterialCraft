//! The shortcuts page: every action of the table, grouped, with its bindings; a click on a
//! binding records a new one from the keyboard, mouse or pen (WS-12).

use eframe::egui::{self, Color32, RichText, Ui};

use super::capture::{Captured, capture};
use super::chord::Chord;
use super::table::{ActionSpec, Kind, ShortcutTable};
use crate::gui_theme;

/// A change the user made on the page; the owner applies it to the table and saves.
#[derive(Clone, Debug, PartialEq)]
pub enum PageEdit {
    Set(&'static str, Vec<Chord>),
    Reset(&'static str),
    ResetAll,
}

/// A binding being recorded: the action, and which of its bindings is replaced (`None` adds one).
#[derive(Clone, Copy, Debug)]
struct Recording {
    id: &'static str,
    slot: Option<usize>,
    kind: Kind,
    /// False during the frame of the click that started it, so that click isn't recorded.
    armed: bool,
}

/// The page's own state: the search text and the binding being recorded.
#[derive(Default)]
pub struct ShortcutsPage {
    search: String,
    recording: Option<Recording>,
}

/// The actions the page lists for `search` (matching label, group, id or a binding), in table order.
pub fn rows(table: &ShortcutTable, search: &str) -> Vec<&'static ActionSpec> {
    let q = search.trim().to_lowercase();
    table
        .specs()
        .iter()
        .filter(|s| {
            q.is_empty()
                || s.label.to_lowercase().contains(&q)
                || s.group.to_lowercase().contains(&q)
                || s.id.contains(&q)
                || table.label(s.id).to_lowercase().contains(&q)
        })
        .collect()
}

impl ShortcutsPage {
    /// True while the page waits for input: the screen must not run shortcuts meanwhile.
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn show(&mut self, ui: &mut Ui, table: &ShortcutTable) -> Option<PageEdit> {
        let mut edit = self.record(ui, table);
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search actions or keys").desired_width(ui.available_width() - 90.0));
            if ui.button("Reset all").on_hover_text("Restore every default binding").clicked() {
                edit = Some(PageEdit::ResetAll);
            }
        });
        if self.recording.is_some() {
            ui.label(RichText::new("Press the new key, button or wheel. Esc cancels, Backspace removes.").color(gui_theme::ACCENT));
        }
        ui.separator();
        let rows = rows(table, &self.search);
        egui::ScrollArea::vertical().id_salt("shortcuts_page").auto_shrink([false, false]).show(ui, |ui| {
            let mut group = "";
            for spec in rows {
                if spec.group != group {
                    group = spec.group;
                    ui.add_space(6.0);
                    ui.label(RichText::new(group).strong().color(gui_theme::HEADER_TEXT));
                }
                if let Some(e) = self.row(ui, table, spec) {
                    edit = Some(e);
                }
            }
        });
        edit
    }

    fn row(&mut self, ui: &mut Ui, table: &ShortcutTable, spec: &'static ActionSpec) -> Option<PageEdit> {
        let mut edit = None;
        ui.horizontal(|ui| {
            let conflicts = table.conflicts(spec.id);
            let label = ui.add_sized([170.0, 18.0], egui::Label::new(RichText::new(spec.label).color(gui_theme::LABEL_TEXT)).truncate());
            if !conflicts.is_empty() {
                let names: Vec<&str> = conflicts.iter().map(|s| s.label).collect();
                label.on_hover_text(format!("Same binding as: {}", names.join(", ")));
                ui.colored_label(Color32::from_rgb(230, 170, 70), "⚠");
            }
            for (i, chord) in table.bindings(spec.id).iter().enumerate() {
                let active = self.recording.is_some_and(|r| r.id == spec.id && r.slot == Some(i));
                let text = if active { "…".to_owned() } else { chord.label() };
                if ui.selectable_label(active, text).on_hover_text("Click to change").clicked() {
                    self.recording = Some(Recording { id: spec.id, slot: Some(i), kind: spec.kind, armed: false });
                }
            }
            let adding = self.recording.is_some_and(|r| r.id == spec.id && r.slot.is_none());
            if ui.selectable_label(adding, if adding { "…" } else { "+" }).on_hover_text("Add a binding").clicked() {
                self.recording = Some(Recording { id: spec.id, slot: None, kind: spec.kind, armed: false });
            }
            if !table.is_default(spec.id) && ui.small_button("↺").on_hover_text("Restore the default").clicked() {
                edit = Some(PageEdit::Reset(spec.id));
            }
        });
        edit
    }

    /// Feeds this frame's input to the binding being recorded.
    fn record(&mut self, ui: &mut Ui, table: &ShortcutTable) -> Option<PageEdit> {
        let rec = self.recording?;
        if !rec.armed {
            self.recording = Some(Recording { armed: true, ..rec });
            return None;
        }
        let got = ui.input_mut(|i| {
            let got = capture(&i.events, rec.kind);
            if got != Captured::Nothing {
                // The input belongs to the page, not to the widgets or the screen.
                i.events.clear();
            }
            got
        });
        let mut chords: Vec<Chord> = table.bindings(rec.id).to_vec();
        match got {
            Captured::Nothing => return None,
            Captured::Cancel => {}
            Captured::Clear => {
                if let Some(i) = rec.slot.filter(|i| *i < chords.len()) {
                    chords.remove(i);
                }
            }
            Captured::Chord(c) => match rec.slot.and_then(|i| chords.get_mut(i)) {
                Some(slot) => *slot = c,
                None => chords.push(c),
            },
        }
        self.recording = None;
        (got != Captured::Cancel).then_some(PageEdit::Set(rec.id, chords))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    const SPECS: &[ActionSpec] = &[
        ActionSpec { id: "open", label: "Open", group: "File", kind: Kind::Press, defaults: &["Mod+O"] },
        ActionSpec { id: "orbit", label: "Orbit", group: "Navigation", kind: Kind::Gesture, defaults: &["Alt+LeftDrag"] },
        ActionSpec { id: "unbound", label: "Unbound", group: "File", kind: Kind::Press, defaults: &[] },
    ];

    #[test]
    fn the_page_lists_every_action_and_search_narrows_it() {
        let (t, _) = ShortcutTable::new(SPECS, &BTreeMap::new());
        assert_eq!(rows(&t, "").len(), SPECS.len());
        assert_eq!(rows(&t, "orb").iter().map(|s| s.id).collect::<Vec<_>>(), vec!["orbit"]);
        assert_eq!(rows(&t, "navigation").len(), 1);
        let ctrl_o = Chord::parse("Mod+O").expect("chord").label().to_lowercase();
        assert_eq!(rows(&t, &ctrl_o).iter().map(|s| s.id).collect::<Vec<_>>(), vec!["open"]);
    }
}
