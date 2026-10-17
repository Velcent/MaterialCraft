//! [`History`]: a linear list of named steps with a cursor, a coalescing rule and a byte budget.

use super::budget::Retained;
use super::step::{Step, StepInfo, rollback};
use super::{CoalesceKey, Command};
use std::time::SystemTime;

/// Default budget for the memory only the history keeps alive (old tiles, removed layers): 1 GiB.
pub const DEFAULT_BUDGET: usize = 1 << 30;

/// One linear undo history. See the module docs.
pub struct History<C: Command> {
    /// `steps[..cursor]` are done (their ops undo them), `steps[cursor..]` undone (their ops redo them).
    steps: Vec<Step<C>>,
    cursor: usize,
    /// The step before the cursor still takes edits with its key.
    open: bool,
    budget: usize,
    retained: Retained,
    next_serial: u64,
    dropped: usize,
}

impl<C: Command> Default for History<C> {
    fn default() -> Self {
        History::new(DEFAULT_BUDGET)
    }
}

impl<C: Command> History<C> {
    /// An empty history that keeps at most `budget` bytes alive beyond what the document holds.
    pub fn new(budget: usize) -> History<C> {
        History { steps: Vec::new(), cursor: 0, open: false, budget, retained: Retained::default(), next_serial: 0, dropped: 0 }
    }

    /// Applies `edit` to `target` and records it as a step (or joins the open step with the same `key`).
    pub fn apply(&mut self, target: &mut C::Target, label: impl Into<String>, key: Option<CoalesceKey>, edit: C) -> Result<(), C::Error> {
        self.apply_all(target, label, key, vec![edit])
    }

    /// Applies `edits` in order as one step (e.g. a mesh re-import that also replaces texture sets). If one
    /// fails, the ones before it are rolled back, nothing is recorded and the error is returned.
    pub fn apply_all(&mut self, target: &mut C::Target, label: impl Into<String>, key: Option<CoalesceKey>, edits: Vec<C>) -> Result<(), C::Error> {
        let mut inverses = Vec::with_capacity(edits.len());
        for edit in edits {
            match edit.apply(target) {
                Ok(inv) => inverses.push(inv),
                Err(e) => {
                    rollback(target, inverses);
                    return Err(e);
                }
            }
        }
        self.record_all(target, label, key, inverses);
        Ok(())
    }

    /// Records an edit that was already made to `target` by its `inverse` (e.g. from a
    /// [`crate::paint::edit::TileCapture`] after painting straight into an image).
    pub fn record(&mut self, target: &C::Target, label: impl Into<String>, key: Option<CoalesceKey>, inverse: C) {
        self.record_all(target, label, key, vec![inverse]);
    }

    /// [`Self::record`] for several edits, `inverses` in the order the edits were made.
    pub fn record_all(&mut self, target: &C::Target, label: impl Into<String>, key: Option<CoalesceKey>, inverses: Vec<C>) {
        self.drop_redo();
        let joins = key.is_some() && self.open && self.steps.last().is_some_and(|s| s.key == key);
        match self.steps.last_mut() {
            Some(step) if joins => {
                self.retained.remove(&step.ops);
                step.absorb(inverses);
                self.retained.add(&step.ops);
            }
            _ => {
                self.retained.add(&inverses);
                let serial = self.next_serial;
                self.next_serial += 1;
                self.steps.push(Step { serial, label: label.into(), time: SystemTime::now(), key, ops: inverses });
                self.cursor = self.steps.len();
            }
        }
        self.open = key.is_some();
        self.trim(target);
    }

    /// Ends the open step: the next edit starts a new one even with the same key (call at the end of a drag or
    /// a stroke).
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Steps back once. Returns false if there was nothing to undo. If the step cannot be undone (the target
    /// was changed outside the history), the target is left as it was, that step and every older one are
    /// dropped, and the error is returned.
    pub fn undo(&mut self, target: &mut C::Target) -> Result<bool, C::Error> {
        let moved = self.step_back(target)?;
        self.trim(target);
        Ok(moved)
    }

    /// Steps forward once. Returns false if there was nothing to redo. On error the target is left as it was
    /// and that step and every newer one are dropped.
    pub fn redo(&mut self, target: &mut C::Target) -> Result<bool, C::Error> {
        let moved = self.step_forward(target)?;
        self.trim(target);
        Ok(moved)
    }

    fn step_back(&mut self, target: &mut C::Target) -> Result<bool, C::Error> {
        self.open = false;
        let Some(i) = self.cursor.checked_sub(1) else {
            return Ok(false);
        };
        self.cross(target, i)?;
        self.cursor = i;
        Ok(true)
    }

    fn step_forward(&mut self, target: &mut C::Target) -> Result<bool, C::Error> {
        self.open = false;
        if self.cursor >= self.steps.len() {
            return Ok(false);
        }
        self.cross(target, self.cursor)?;
        self.cursor += 1;
        Ok(true)
    }

    /// Undoes or redoes until `done` steps are done (the timeline's "click a block": `done = index + 1` returns
    /// to the state after step `index`, `0` to the oldest state kept). Values past the end mean all.
    pub fn jump_to(&mut self, target: &mut C::Target, done: usize) -> Result<(), C::Error> {
        let done = done.min(self.steps.len());
        // Trim once at the end: dropping steps on the way would shift the indices.
        let result = self.walk_to(target, done);
        self.open = false;
        self.trim(target);
        result
    }

    fn walk_to(&mut self, target: &mut C::Target, done: usize) -> Result<(), C::Error> {
        while self.cursor > done && self.step_back(target)? {}
        while self.cursor < done.min(self.steps.len()) && self.step_forward(target)? {}
        Ok(())
    }

    fn cross(&mut self, target: &mut C::Target, i: usize) -> Result<(), C::Error> {
        let Some(step) = self.steps.get_mut(i) else {
            return Ok(());
        };
        self.retained.remove(&step.ops);
        let result = step.cross(target);
        if result.is_ok() {
            self.retained.add(&step.ops);
        } else {
            // The step lost the ops it had applied (the rest are released above); everything beyond it in that
            // direction is unreachable now.
            step.ops.clear();
            if i < self.cursor {
                self.drop_range(0, i + 1);
            } else {
                self.drop_range(i, self.steps.len());
            }
        }
        result
    }

    fn drop_redo(&mut self) {
        self.drop_range(self.cursor, self.steps.len());
    }

    /// Removes `steps[start..end]` and releases their memory.
    fn drop_range(&mut self, start: usize, end: usize) {
        let end = end.min(self.steps.len());
        if start >= end {
            return;
        }
        for step in self.steps.drain(start..end) {
            self.retained.remove(&step.ops);
            self.dropped += 1;
        }
        let below = self.cursor.min(end).saturating_sub(start);
        self.cursor -= below;
    }

    /// Drops the oldest steps until the memory only the history holds fits the budget. The newest step is kept
    /// even if it alone is over budget, so the last edit can always be undone.
    fn trim(&mut self, target: &C::Target) {
        while self.steps.len() > 1 && self.retained.total() > self.budget && self.retained.beyond::<C>(target) > self.budget {
            if self.cursor > 0 {
                self.drop_range(0, 1);
            } else {
                self.drop_range(self.steps.len().saturating_sub(1), self.steps.len());
            }
        }
    }

    /// Changes the budget and drops old steps to meet it.
    pub fn set_budget(&mut self, target: &C::Target, budget: usize) {
        self.budget = budget;
        self.trim(target);
    }

    pub fn budget(&self) -> usize {
        self.budget
    }

    /// Bytes only the history keeps alive (not shared with `target`); at most the budget unless the newest
    /// step alone is larger.
    pub fn retained_bytes(&self, target: &C::Target) -> usize {
        self.retained.beyond::<C>(target)
    }

    /// Forgets every step (e.g. when the document is closed or replaced).
    pub fn clear(&mut self) {
        self.dropped += self.steps.len();
        self.steps.clear();
        self.cursor = 0;
        self.open = false;
        self.retained.clear();
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_redo(&self) -> bool {
        self.cursor < self.steps.len()
    }

    /// Label of the step [`Self::undo`] would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.steps.get(self.cursor.checked_sub(1)?).map(|s| s.label.as_str())
    }

    /// Label of the step [`Self::redo`] would redo.
    pub fn redo_label(&self) -> Option<&str> {
        self.steps.get(self.cursor).map(|s| s.label.as_str())
    }

    /// Number of steps kept, done and undone.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Number of done steps: the current state is after step `done() - 1`.
    pub fn done(&self) -> usize {
        self.cursor
    }

    /// Steps dropped so far (budget, new edits after undo, failures, [`Self::clear`]).
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    /// Every step, oldest first, for the timeline.
    pub fn steps(&self) -> impl ExactSizeIterator<Item = StepInfo<'_>> {
        self.steps.iter().enumerate().map(|(i, s)| StepInfo { serial: s.serial, label: &s.label, time: s.time, done: i < self.cursor })
    }
}
