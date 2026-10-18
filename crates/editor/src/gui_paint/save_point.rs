//! [`SavePoint`]: where in the history the document was last saved, so the Paint screen knows whether it has
//! unsaved changes. The document is dirty when the history's current state differs from the saved one; undoing
//! back to the saved state makes it clean again.
//!
//! A state is the gap between two steps. Steps keep their serial for their lifetime, so the saved state is named by
//! the step just before it and, once one is recorded from there, the step just after it. Either neighbour still in
//! the history identifies the state, so dropping old steps for the memory budget does not make a document dirty.

use materialcraft::history::{Command, History};

/// A state of the history, named by its neighbouring steps' serials.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SavePoint {
    /// The step that led to the state (the last done step).
    before: Option<u64>,
    /// The step recorded or kept right after the state (the first undone step, or the first edit made from it).
    after: Option<u64>,
}

impl SavePoint {
    /// The current state of `history`.
    pub fn of<C: Command>(history: &History<C>) -> SavePoint {
        let mut point = SavePoint::default();
        for s in history.steps() {
            if s.done {
                point.before = Some(s.serial);
            } else {
                point.after = Some(s.serial);
                break;
            }
        }
        point
    }

    /// True if `history` is at this state.
    pub fn is_current<C: Command>(&self, history: &History<C>) -> bool {
        let now = SavePoint::of(history);
        match (now.before, now.after) {
            (Some(b), _) if Some(b) == self.before => true,
            (_, Some(a)) if Some(a) == self.after => true,
            // No step on either side: nothing was ever recorded, or only from here.
            (None, None) => self.before.is_none() && self.after.is_none(),
            _ => false,
        }
    }

    /// Call after a step was recorded: if the history was at this state just before (`was_current`), the new step
    /// leads away from it and names it from now on.
    pub fn note_recorded<C: Command>(&mut self, was_current: bool, history: &History<C>) {
        let now = SavePoint::of(history);
        if was_current && now.before != self.before {
            self.after = now.before;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::history::PaintHistory;
    use materialcraft::paint::Document;
    use materialcraft::paint::edit::{Edit, Transaction};
    use materialcraft::paint::height_normal::HeightFilter;

    /// Records one step named `name` (it flips the document's height filter: any recorded change will do).
    fn edit(doc: &mut Document, h: &mut PaintHistory, point: &mut SavePoint, name: &str) {
        let was = point.is_current(h);
        let flipped = if doc.height_filter == HeightFilter::Sobel { HeightFilter::Backward } else { HeightFilter::Sobel };
        let mut tx = Transaction::new(doc);
        tx.apply(Edit::SetHeightFilter(flipped)).unwrap();
        let inverses = tx.commit();
        h.record_all(doc, name, None, inverses);
        point.note_recorded(was, h);
    }

    #[test]
    fn clean_until_an_edit_and_again_after_undoing_it() {
        let (mut doc, mut h) = (Document::default(), PaintHistory::default());
        let mut saved = SavePoint::of(&h);
        assert!(saved.is_current(&h), "a new document is clean");
        edit(&mut doc, &mut h, &mut saved, "a");
        assert!(!saved.is_current(&h));
        h.undo(&mut doc).unwrap();
        assert!(saved.is_current(&h), "undone back to the saved state");
        h.redo(&mut doc).unwrap();
        assert!(!saved.is_current(&h));
    }

    #[test]
    fn saved_in_the_middle_of_the_history() {
        let (mut doc, mut h) = (Document::default(), PaintHistory::default());
        let mut saved = SavePoint::of(&h);
        for n in ["a", "bb", "c"] {
            edit(&mut doc, &mut h, &mut saved, n);
        }
        h.undo(&mut doc).unwrap();
        saved = SavePoint::of(&h);
        assert!(saved.is_current(&h));
        h.redo(&mut doc).unwrap();
        assert!(!saved.is_current(&h), "redo leaves the saved state");
        h.undo(&mut doc).unwrap();
        h.undo(&mut doc).unwrap();
        assert!(!saved.is_current(&h), "undo past it too");
        h.redo(&mut doc).unwrap();
        assert!(saved.is_current(&h));
        // A new edit from the saved state drops the undone step; undoing it comes back.
        edit(&mut doc, &mut h, &mut saved, "dd");
        assert!(!saved.is_current(&h));
        h.undo(&mut doc).unwrap();
        assert!(saved.is_current(&h));
        // A new edit from elsewhere replaces the redo branch: the saved state is out of reach.
        h.undo(&mut doc).unwrap();
        edit(&mut doc, &mut h, &mut saved, "e");
        h.undo(&mut doc).unwrap();
        assert!(!saved.is_current(&h));
    }

    #[test]
    fn a_timeline_jump_back_to_the_saved_state_is_clean() {
        let (mut doc, mut h) = (Document::default(), PaintHistory::default());
        let mut saved = SavePoint::of(&h);
        edit(&mut doc, &mut h, &mut saved, "a");
        saved = SavePoint::of(&h);
        edit(&mut doc, &mut h, &mut saved, "bb");
        edit(&mut doc, &mut h, &mut saved, "c");
        h.jump_to(&mut doc, 0).unwrap();
        assert!(!saved.is_current(&h));
        h.jump_to(&mut doc, 1).unwrap();
        assert!(saved.is_current(&h));
    }
}
