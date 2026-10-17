//! The bounded history of entries and the queries the log panel runs on it (LG-08).

use std::collections::VecDeque;

use super::{Entry, Filter, Level};

/// A stable id of an entry within one [`RingBuffer`]: ids grow by one per push and are never
/// reused, also across [`RingBuffer::clear`], so the panel can keep a selection or a scroll anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryId(pub u64);

/// Entries per level, for badges and summaries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    pub debug: usize,
    pub info: usize,
    pub warning: usize,
    pub error: usize,
}

impl Counts {
    pub fn get(&self, level: Level) -> usize {
        match level {
            Level::Debug => self.debug,
            Level::Info => self.info,
            Level::Warning => self.warning,
            Level::Error => self.error,
        }
    }

    pub fn total(&self) -> usize {
        self.debug + self.info + self.warning + self.error
    }

    fn add(&mut self, level: Level) {
        let n = match level {
            Level::Debug => &mut self.debug,
            Level::Info => &mut self.info,
            Level::Warning => &mut self.warning,
            Level::Error => &mut self.error,
        };
        *n = n.saturating_add(1);
    }
}

/// A first-in, first-out buffer of at most `capacity` entries. When full, a push drops the
/// oldest entry, so memory stays bounded and the newest entries are always kept.
#[derive(Debug, Clone)]
pub struct RingBuffer {
    entries: VecDeque<Entry>,
    capacity: usize,
    /// Id of `entries.front()`, or of the next push when empty.
    first_id: u64,
    dropped: u64,
}

impl RingBuffer {
    /// The panel history's default size.
    pub const DEFAULT_CAPACITY: usize = 10_000;

    /// An empty buffer holding up to `capacity` entries (at least one).
    pub fn new(capacity: usize) -> RingBuffer {
        RingBuffer { entries: VecDeque::new(), capacity: capacity.max(1), first_id: 0, dropped: 0 }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many entries were pushed out by newer ones since the buffer was made.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The id the next push gets.
    pub fn next_id(&self) -> EntryId {
        EntryId(self.first_id.saturating_add(self.entries.len() as u64))
    }

    /// Append `entry`, dropping the oldest one if the buffer is full; returns the new entry's id.
    pub fn push(&mut self, entry: Entry) -> EntryId {
        if self.entries.len() >= self.capacity && self.entries.pop_front().is_some() {
            self.first_id = self.first_id.saturating_add(1);
            self.dropped = self.dropped.saturating_add(1);
        }
        let id = self.next_id();
        self.entries.push_back(entry);
        id
    }

    /// Remove every entry (LG-07). Ids keep counting, so old ids don't point at new entries.
    pub fn clear(&mut self) {
        self.first_id = self.next_id().0;
        self.entries.clear();
    }

    /// The entry with `id`, if it is still in the buffer.
    pub fn get(&self, id: EntryId) -> Option<&Entry> {
        let offset = usize::try_from(id.0.checked_sub(self.first_id)?).ok()?;
        self.entries.get(offset)
    }

    /// The newest entry.
    pub fn last(&self) -> Option<(EntryId, &Entry)> {
        let entry = self.entries.back()?;
        Some((EntryId(self.next_id().0.saturating_sub(1)), entry))
    }

    /// Every entry with its id, oldest first.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (EntryId, &Entry)> + '_ {
        let first = self.first_id;
        self.entries.iter().enumerate().map(move |(i, e)| (EntryId(first.saturating_add(i as u64)), e))
    }

    /// The entries themselves, oldest first, consuming the buffer.
    pub fn into_entries(self) -> impl Iterator<Item = Entry> {
        self.entries.into_iter()
    }

    /// Entries with an id of at least `id`, oldest first: what arrived since the panel last looked.
    pub fn since(&self, id: EntryId) -> impl Iterator<Item = (EntryId, &Entry)> + '_ {
        let skip = usize::try_from(id.0.saturating_sub(self.first_id)).unwrap_or(usize::MAX);
        self.iter().skip(skip)
    }

    /// Entries `filter` accepts, oldest first.
    pub fn query<'a>(&'a self, filter: &'a Filter) -> impl DoubleEndedIterator<Item = (EntryId, &'a Entry)> + 'a {
        self.iter().filter(move |(_, e)| filter.matches(e))
    }

    /// Per-level counts of the entries `filter` accepts.
    pub fn counts(&self, filter: &Filter) -> Counts {
        let mut counts = Counts::default();
        for (_, e) in self.query(filter) {
            counts.add(e.level);
        }
        counts
    }

    /// The entries `filter` accepts as text, one per line, for copying the log (LG-06).
    pub fn to_text(&self, filter: &Filter) -> String {
        let mut out = String::new();
        for (_, e) in self.query(filter) {
            out.push_str(&e.to_string());
            out.push('\n');
        }
        out
    }
}

impl Default for RingBuffer {
    fn default() -> RingBuffer {
        RingBuffer::new(RingBuffer::DEFAULT_CAPACITY)
    }
}
