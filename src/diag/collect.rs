//! The sink every producer thread writes to, handed over to the UI in batches (LG-08).

use std::sync::{Mutex, PoisonError};

use super::{Area, Entry, Level, RingBuffer, Sink};

/// A thread-safe [`Sink`] that keeps entries in a bounded inbox until the owner moves them into
/// its history with [`Collector::drain_into`] (once per frame in the editor).
///
/// `emit` holds a lock only for one push, never waits on the reader for longer than a buffer
/// swap, and drops the oldest waiting entry when the inbox is full, so a burst can't stall a
/// worker or grow memory. A poisoned lock is taken over rather than propagated: diagnostics must
/// keep working after another thread panicked.
#[derive(Debug)]
pub struct Collector {
    inbox: Mutex<RingBuffer>,
    capacity: usize,
}

impl Collector {
    /// The inbox's default size: more than a frame's worth of messages in any sane burst.
    pub const DEFAULT_CAPACITY: usize = 4096;

    /// A collector whose inbox holds up to `capacity` entries (at least one).
    pub fn new(capacity: usize) -> Collector {
        let inbox = RingBuffer::new(capacity);
        let capacity = inbox.capacity();
        Collector { inbox: Mutex::new(inbox), capacity }
    }

    /// Entries waiting for the next drain.
    pub fn pending(&self) -> usize {
        self.inbox.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Move every waiting entry into `history`, oldest first. If the inbox overflowed since the
    /// last drain, a warning saying how many entries were lost comes first. Returns how many
    /// entries were added to `history`, the warning included.
    pub fn drain_into(&self, history: &mut RingBuffer) -> usize {
        let batch = {
            let mut inbox = self.inbox.lock().unwrap_or_else(PoisonError::into_inner);
            std::mem::replace(&mut *inbox, RingBuffer::new(self.capacity))
        };
        let mut added = 0;
        if batch.dropped() > 0 {
            let mut note = Entry::new(Level::Warning, Area::General, format!("{} messages were dropped during a burst", batch.dropped()));
            if let Some((_, first)) = batch.iter().next() {
                note.time = first.time;
            }
            history.push(note);
            added += 1;
        }
        for entry in batch.into_entries() {
            history.push(entry);
            added += 1;
        }
        added
    }
}

impl Default for Collector {
    fn default() -> Collector {
        Collector::new(Collector::DEFAULT_CAPACITY)
    }
}

impl Sink for Collector {
    fn emit(&self, entry: Entry) {
        self.inbox.lock().unwrap_or_else(PoisonError::into_inner).push(entry);
    }
}
