//! [`Step`]: one named entry of the timeline, and what the timeline shows of it ([`StepInfo`]).

use super::Command;
use std::time::SystemTime;

/// Groups consecutive edits into one step: every edit recorded with the same key, while the step is still open
/// (no undo, redo, jump or [`super::History::close`] in between), joins the step before it. One slider drag or
/// one brush stroke uses one key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoalesceKey(pub u64);

/// What the timeline shows of a step.
#[derive(Debug, Clone, PartialEq)]
pub struct StepInfo<'a> {
    /// Stable for the step's lifetime (indices shift when old steps are dropped).
    pub serial: u64,
    pub label: &'a str,
    /// When the step was first recorded.
    pub time: SystemTime,
    /// True for steps up to the current state, false for undone ones.
    pub done: bool,
}

pub(super) struct Step<C> {
    pub(super) serial: u64,
    pub(super) label: String,
    pub(super) time: SystemTime,
    pub(super) key: Option<CoalesceKey>,
    /// Applied last to first, these take the document across the step: back while the step is done, forward
    /// while it is undone. Applying them yields the commands for the other direction, in the order applied.
    pub(super) ops: Vec<C>,
}

impl<C: Command> Step<C> {
    /// Folds the inverses of edits applied after this step's into it.
    pub(super) fn absorb(&mut self, later: Vec<C>) {
        for op in later {
            let rest = match self.ops.last_mut() {
                Some(last) => last.absorb(op),
                None => Some(op),
            };
            self.ops.extend(rest);
        }
    }

    /// Applies the ops last to first; on success they are replaced by the ops for the other direction. On error
    /// the ops already applied are rolled back, the target is as before, and the step can no longer be crossed.
    pub(super) fn cross(&mut self, target: &mut C::Target) -> Result<(), C::Error> {
        let mut inverse = Vec::with_capacity(self.ops.len());
        while let Some(op) = self.ops.pop() {
            match op.apply(target) {
                Ok(inv) => inverse.push(inv),
                Err(e) => {
                    rollback(target, inverse);
                    return Err(e);
                }
            }
        }
        self.ops = inverse;
        Ok(())
    }
}

/// Applies `inverses` (in the order their edits were applied) last to first, putting the target back.
pub(super) fn rollback<C: Command>(target: &mut C::Target, inverses: Vec<C>) {
    for op in inverses.into_iter().rev() {
        // An inverse of an edit that was just applied cannot fail unless the command type is broken; there is
        // nothing better to do with such an error than to go on restoring the rest.
        let _ = op.apply(target);
    }
}
