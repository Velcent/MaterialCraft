//! [`Retained`]: the bytes of the shared allocations the history keeps alive, each counted once.
//!
//! The design follows PhotoCraft's `ops::tiles` (count references per allocation, so a tile shared by several
//! steps costs once and is released with its last reference), adapted to commands instead of snapshots.

use super::Command;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub(super) struct Retained {
    /// Allocation id → (references from history steps, bytes).
    refs: HashMap<usize, (usize, usize)>,
    bytes: usize,
}

impl Retained {
    pub(super) fn add<C: Command>(&mut self, ops: &[C]) {
        for op in ops {
            op.retained(&mut |id, bytes| {
                let entry = self.refs.entry(id).or_insert_with(|| {
                    self.bytes = self.bytes.saturating_add(bytes);
                    (0, bytes)
                });
                entry.0 += 1;
            });
        }
    }

    pub(super) fn remove<C: Command>(&mut self, ops: &[C]) {
        for op in ops {
            op.retained(&mut |id, _| {
                if let Some(entry) = self.refs.get_mut(&id) {
                    entry.0 = entry.0.saturating_sub(1);
                    if entry.0 == 0 {
                        self.bytes = self.bytes.saturating_sub(entry.1);
                        self.refs.remove(&id);
                    }
                }
            });
        }
    }

    /// Bytes held by any step, including allocations the target shares.
    pub(super) fn total(&self) -> usize {
        self.bytes
    }

    /// Bytes only the history keeps alive: [`Self::total`] minus what `target` holds as well.
    pub(super) fn beyond<C: Command>(&self, target: &C::Target) -> usize {
        if self.bytes == 0 {
            return 0;
        }
        let mut shared = 0usize;
        let mut seen = HashSet::new();
        C::target_retained(target, &mut |id, _| {
            if let Some((_, bytes)) = self.refs.get(&id)
                && seen.insert(id)
            {
                shared = shared.saturating_add(*bytes);
            }
        });
        self.bytes.saturating_sub(shared)
    }

    pub(super) fn clear(&mut self) {
        self.refs.clear();
        self.bytes = 0;
    }
}
