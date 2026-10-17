//! Paint documents in the history: [`Edit`] as a [`Command`] whose shared memory is its tiles.

use super::Command;
use crate::paint::Document;
use crate::paint::edit::{Edit, EditError, document_tiles};

impl Command for Edit {
    type Target = Document;
    type Error = EditError;

    fn apply(self, doc: &mut Document) -> Result<Edit, EditError> {
        Edit::apply(self, doc)
    }

    fn absorb(&mut self, later: Edit) -> Option<Edit> {
        Edit::absorb(self, later)
    }

    fn retained(&self, f: &mut dyn FnMut(usize, usize)) {
        self.for_each_tile(&mut |t| f(t.id(), t.len()));
    }

    fn target_retained(doc: &Document, f: &mut dyn FnMut(usize, usize)) {
        document_tiles(doc, &mut |t| f(t.id(), t.len()));
    }
}
