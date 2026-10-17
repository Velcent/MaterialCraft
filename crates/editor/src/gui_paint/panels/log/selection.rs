//! [`Selection`]: the log rows the user picked for copying (click, Ctrl+click, Shift+click).

use std::collections::BTreeSet;

use materialcraft::diag::EntryId;

use super::index::LogIndex;

/// Selected entries by id, so a selection survives new entries and filter changes.
#[derive(Debug, Default)]
pub struct Selection {
    ids: BTreeSet<EntryId>,
    anchor: Option<EntryId>,
}

/// How a row was clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    /// Select only this row.
    Plain,
    /// Add or remove this row (Ctrl / Cmd).
    Toggle,
    /// Select the listed rows from the last clicked one to this one (Shift).
    Range,
}

impl Selection {
    pub fn contains(&self, id: EntryId) -> bool {
        self.ids.contains(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Selected ids, oldest first.
    pub fn ids(&self) -> impl Iterator<Item = EntryId> + '_ {
        self.ids.iter().copied()
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
    }

    /// Applies a click on the row of `id`; `index` gives the order of the listed rows.
    pub fn click(&mut self, id: EntryId, how: Click, index: &LogIndex) {
        match how {
            Click::Plain => {
                self.ids.clear();
                self.ids.insert(id);
                self.anchor = Some(id);
            }
            Click::Toggle => {
                if !self.ids.remove(&id) {
                    self.ids.insert(id);
                }
                self.anchor = Some(id);
            }
            Click::Range => {
                let range = self.anchor.and_then(|a| Some((index.position(a)?, index.position(id)?)));
                match range {
                    Some((a, b)) => {
                        self.ids.clear();
                        self.ids.extend(index.shown().range(a.min(b)..=a.max(b)).copied());
                    }
                    None => self.click(id, Click::Plain, index),
                }
            }
        }
    }
}
