//! [`LogIndex`]: which history entries the log panel lists, kept up to date incrementally so a
//! frame costs time for the entries that arrived, not for the whole history (LG-08).

use std::collections::VecDeque;

use materialcraft::diag::{Counts, EntryId, Filter, Level, RingBuffer, Tab};

/// What the index remembers of every entry it has seen: enough for the tab badges.
#[derive(Clone, Copy, Debug)]
struct Seen {
    id: EntryId,
    tab: Tab,
    level: Level,
}

/// The entries of a [`RingBuffer`] that pass the panel's [`Filter`], by id, oldest first, plus
/// per-tab counts. [`LogIndex::sync`] once a frame picks up new entries and forgets the ones the
/// history dropped; changing the filter rescans the history once.
#[derive(Debug)]
pub struct LogIndex {
    filter: Filter,
    /// Every entry since the last clear that is still in the history, oldest first.
    seen: VecDeque<Seen>,
    /// The ids of `seen` that pass `filter`.
    shown: VecDeque<EntryId>,
    counts: [Counts; 3],
    /// The first id not looked at yet.
    next: EntryId,
    /// Entries before this id were cleared from the panel (the history and log file keep them).
    cleared: EntryId,
}

impl Default for LogIndex {
    fn default() -> LogIndex {
        LogIndex::new(Filter::default())
    }
}

impl LogIndex {
    pub fn new(filter: Filter) -> LogIndex {
        LogIndex { filter, seen: VecDeque::new(), shown: VecDeque::new(), counts: [Counts::default(); 3], next: EntryId(0), cleared: EntryId(0) }
    }

    pub fn filter(&self) -> &Filter {
        &self.filter
    }

    /// Shows what `filter` accepts from now on; rescans `history` if the filter changed.
    pub fn set_filter(&mut self, filter: Filter, history: &RingBuffer) {
        if filter == self.filter {
            return;
        }
        self.filter = filter;
        self.shown.clear();
        for s in &self.seen {
            if history.get(s.id).is_some_and(|e| self.filter.matches(e)) {
                self.shown.push_back(s.id);
            }
        }
    }

    /// Picks up the entries `history` received since the last call and forgets those it dropped.
    /// Returns how many new entries pass the filter.
    pub fn sync(&mut self, history: &RingBuffer) -> usize {
        let first = history.iter().next().map_or(history.next_id(), |(id, _)| id);
        if history.next_id() < self.next {
            // A different history (ids went backwards): start over.
            *self = LogIndex::new(std::mem::take(&mut self.filter));
        }
        while let Some(s) = self.seen.front().copied().filter(|s| s.id < first) {
            self.seen.pop_front();
            if let Some(c) = self.counts.get_mut(tab_index(s.tab)) {
                c.remove(s.level);
            }
        }
        while self.shown.front().is_some_and(|id| *id < first) {
            self.shown.pop_front();
        }
        let mut added = 0;
        for (id, e) in history.since(self.next.max(self.cleared)) {
            let tab = e.area.tab();
            self.seen.push_back(Seen { id, tab, level: e.level });
            if let Some(c) = self.counts.get_mut(tab_index(tab)) {
                c.add(e.level);
            }
            if self.filter.matches(e) {
                self.shown.push_back(id);
                added += 1;
            }
        }
        self.next = history.next_id();
        added
    }

    /// Hides everything seen so far; later entries show as usual (LG-07).
    pub fn clear(&mut self) {
        self.cleared = self.next;
        self.seen.clear();
        self.shown.clear();
        self.counts = [Counts::default(); 3];
    }

    /// The ids of the listed entries, oldest first.
    pub fn shown(&self) -> &VecDeque<EntryId> {
        &self.shown
    }

    pub fn len(&self) -> usize {
        self.shown.len()
    }

    /// The position of `id` in the list.
    pub fn position(&self, id: EntryId) -> Option<usize> {
        self.shown.binary_search(&id).ok()
    }

    /// Entries per level in `tab` since the last clear, whatever the filter.
    pub fn counts(&self, tab: Tab) -> Counts {
        self.counts.get(tab_index(tab)).copied().unwrap_or_default()
    }
}

fn tab_index(tab: Tab) -> usize {
    match tab {
        Tab::General => 0,
        Tab::Assets => 1,
        Tab::Bake => 2,
    }
}

/// Per-level counting on [`Counts`], which the core only fills from a query.
trait CountsExt {
    fn add(&mut self, level: Level);
    fn remove(&mut self, level: Level);
}

impl CountsExt for Counts {
    fn add(&mut self, level: Level) {
        let n = slot(self, level);
        *n = n.saturating_add(1);
    }

    fn remove(&mut self, level: Level) {
        let n = slot(self, level);
        *n = n.saturating_sub(1);
    }
}

fn slot(c: &mut Counts, level: Level) -> &mut usize {
    match level {
        Level::Debug => &mut c.debug,
        Level::Info => &mut c.info,
        Level::Warning => &mut c.warning,
        Level::Error => &mut c.error,
    }
}
