//! The Log panel (LG-01 to LG-10): the session's diagnostics as the editor drained them into its
//! history, by tab (General, Assets, Bake; ADR 0006 VQ14), level and search text, with
//! auto-scroll, copy, clear, and entries that go to what they concern.
//!
//! The panel only reads the history: [`LogIndex`] keeps the listed ids up to date at the cost of
//! the new entries, [`list`] lays out only the rows in view, and "Clear" hides what was there
//! (the history, the status bar and the log file keep it).

mod index;
mod list;
mod rows;
mod selection;
mod toolbar;

#[cfg(test)]
mod tests;

use eframe::egui::{self, Ui};
use materialcraft::diag::{EntryId, Filter, Level, LevelMask, RingBuffer, Tab};

use super::PanelEvent;
use index::LogIndex;
use list::Scroll;
use rows::Follow;
use selection::Selection;
use toolbar::{Command, Header};

/// The panel's state between frames.
pub struct LogPanel {
    index: LogIndex,
    tab: Tab,
    levels: LevelMask,
    search: String,
    follow: Follow,
    selection: Selection,
    scroll: Scroll,
    /// Rows laid out in the last frame (only those in view; checked by the burst test).
    laid_out: usize,
}

impl Default for LogPanel {
    fn default() -> LogPanel {
        let levels = LevelMask::at_least(Level::Info);
        LogPanel {
            index: LogIndex::new(Filter::all().levels(levels).in_tab(Tab::General)),
            tab: Tab::General,
            levels,
            search: String::new(),
            follow: Follow::default(),
            selection: Selection::default(),
            scroll: Scroll::default(),
            laid_out: 0,
        }
    }
}

impl LogPanel {
    /// Draws the panel over `history`; going to an entry's target becomes a [`PanelEvent::Reveal`].
    pub fn show(&mut self, ui: &mut Ui, history: &RingBuffer, events: &mut Vec<PanelEvent>) {
        self.update(history);
        let header =
            Header { tab: &mut self.tab, levels: &mut self.levels, search: &mut self.search, follow: &mut self.follow, selected: self.selection.len() };
        let command = toolbar::show(ui, header, &self.index);
        self.update(history);
        match command {
            Some(Command::Copy) => {
                let ids: Vec<EntryId> = if self.selection.is_empty() { self.index.shown().iter().copied().collect() } else { self.selection.ids().collect() };
                ui.ctx().copy_text(text_of(history, ids));
            }
            Some(Command::Clear) => {
                self.index.clear();
                self.selection.clear();
                self.follow.resume();
            }
            Some(Command::JumpToNewest) => self.follow.resume(),
            None => {}
        }
        ui.separator();
        if self.index.len() == 0 {
            self.laid_out = 0;
            ui.weak(if self.search.is_empty() { "No messages." } else { "No messages match the search." });
            return;
        }
        let out = list::show(ui, history, &self.index, &self.selection, &mut self.follow, &mut self.scroll);
        self.laid_out = out.laid_out;
        if let Some((id, how)) = out.click {
            self.selection.click(id, how, &self.index);
        }
        if let Some(ids) = out.copy {
            ui.ctx().copy_text(text_of(history, ids));
        }
        if let Some(target) = out.reveal {
            events.push(PanelEvent::Reveal(target));
        }
        // Ctrl+C over the list copies the selection.
        let copy = ui.ui_contains_pointer() && !self.selection.is_empty() && ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy {
            ui.ctx().copy_text(text_of(history, self.selection.ids().collect()));
        }
    }

    /// Picks up new entries and applies the header's filter.
    fn update(&mut self, history: &RingBuffer) {
        self.index.sync(history);
        let filter = Filter::all().levels(self.levels).in_tab(self.tab).containing(self.search.clone());
        if &filter != self.index.filter() {
            self.index.set_filter(filter, history);
        }
    }
}

/// The entries `ids` as text, one line each (LG-06); entries the history dropped are skipped.
fn text_of(history: &RingBuffer, ids: Vec<EntryId>) -> String {
    let mut out = String::new();
    for e in ids.into_iter().filter_map(|id| history.get(id)) {
        out.push_str(&e.to_string());
        out.push('\n');
    }
    out
}
