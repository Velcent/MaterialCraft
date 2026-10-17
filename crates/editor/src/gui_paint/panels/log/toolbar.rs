//! The log panel's header: tabs with their warning and error counts (LG-04), level toggles
//! (LG-02), search, auto-scroll (LG-05), copy (LG-06) and clear (LG-07).

use eframe::egui::{self, Color32, RichText, Ui};
use materialcraft::diag::{Counts, Level, LevelMask, Tab};

use super::index::LogIndex;
use super::rows::Follow;
use crate::gui_paint::panels::status_bar;

/// A button of the header that acts on the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Copy the selected entries, or every listed one when nothing is selected.
    Copy,
    Clear,
    /// Scroll to the newest entry and resume auto-scroll.
    JumpToNewest,
}

/// The header's settings, edited in place.
pub struct Header<'a> {
    pub tab: &'a mut Tab,
    pub levels: &'a mut LevelMask,
    pub search: &'a mut String,
    pub follow: &'a mut Follow,
    pub selected: usize,
}

/// The panel's levels, most serious first; debug is off unless asked for.
const LEVELS: [(Level, &str); 4] = [(Level::Error, "Errors"), (Level::Warning, "Warnings"), (Level::Info, "Info"), (Level::Debug, "Debug")];

pub fn show(ui: &mut Ui, h: Header, index: &LogIndex) -> Option<Command> {
    let mut command = None;
    ui.horizontal(|ui| {
        for tab in Tab::ALL {
            let counts = index.counts(tab);
            let label = RichText::new(tab_name(tab)).strong();
            if ui.selectable_label(*h.tab == tab, label).clicked() {
                *h.tab = tab;
            }
            badges(ui, &counts);
            ui.add_space(6.0);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let counts = index.counts(*h.tab);
        for (level, name) in LEVELS {
            let on = h.levels.contains(level);
            let n = counts.get(level);
            let text = RichText::new(format!("{name} {n}")).color(if on { status_bar::color(level) } else { Color32::from_gray(110) });
            if ui.selectable_label(on, text).on_hover_text(format!("Show or hide {} messages", level.name())).clicked() {
                *h.levels = h.levels.with(level, !on);
            }
        }
        ui.add(egui::TextEdit::singleline(h.search).hint_text("Search").desired_width(140.0));
        if !h.search.is_empty() && ui.small_button("x").on_hover_text("Clear the search").clicked() {
            h.search.clear();
        }
    });
    ui.horizontal(|ui| {
        if ui.checkbox(&mut h.follow.enabled, "Auto-scroll").on_hover_text("Keep the newest message in view").changed() && h.follow.enabled {
            h.follow.resume();
        }
        if h.follow.is_paused()
            && ui.small_button("Newest").on_hover_text("Auto-scroll is paused while you look further up; jump back to the newest message").clicked()
        {
            command = Some(Command::JumpToNewest);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear").on_hover_text("Remove every message from this panel (the log file keeps them)").clicked() {
                command = Some(Command::Clear);
            }
            let copy_hint = if h.selected > 0 { format!("Copy the {} selected messages", h.selected) } else { "Copy every listed message".to_owned() };
            if ui.button("Copy").on_hover_text(copy_hint).clicked() {
                command = Some(Command::Copy);
            }
        });
    });
    command
}

fn tab_name(tab: Tab) -> &'static str {
    match tab {
        Tab::General => "General",
        Tab::Assets => "Assets",
        Tab::Bake => "Bake",
    }
}

/// Error and warning counts beside a tab, when there are any.
fn badges(ui: &mut Ui, counts: &Counts) {
    for level in [Level::Error, Level::Warning] {
        let n = counts.get(level);
        if n > 0 {
            ui.label(RichText::new(n.to_string()).small().strong().color(status_bar::color(level))).on_hover_text(format!("{n} {}", plural(level, n)));
        }
    }
}

fn plural(level: Level, n: usize) -> &'static str {
    match (level, n == 1) {
        (Level::Error, true) => "error",
        (Level::Error, false) => "errors",
        (Level::Warning, true) => "warning",
        _ => "warnings",
    }
}
