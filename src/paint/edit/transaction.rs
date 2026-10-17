//! [`Transaction`]: a group of edits planned one at a time on the document as it is after the edits before
//! them (a move's index depends on the moves made already), kept or rolled back as a whole.

use super::{Edit, EditError};
use crate::paint::doc::Document;

/// Applies edits to a document and remembers their inverses. [`Self::commit`] hands the inverses over (for
/// [`crate::history::History::record_all`]); dropping the transaction without committing puts the document
/// back as it was, so an operation that fails half-way leaves nothing behind.
pub struct Transaction<'a> {
    doc: &'a mut Document,
    inverses: Vec<Edit>,
}

impl<'a> Transaction<'a> {
    pub fn new(doc: &'a mut Document) -> Transaction<'a> {
        Transaction { doc, inverses: Vec::new() }
    }

    /// The document as it is now, with the edits made so far.
    pub fn doc(&self) -> &Document {
        self.doc
    }

    /// Applies `edit`; on error the document is as before this edit (earlier edits stay until the
    /// transaction is dropped).
    pub fn apply(&mut self, edit: Edit) -> Result<(), EditError> {
        let inverse = edit.apply(self.doc)?;
        self.inverses.push(inverse);
        Ok(())
    }

    /// True if no edit was applied yet.
    pub fn is_empty(&self) -> bool {
        self.inverses.is_empty()
    }

    /// Keeps the edits and returns their inverses, in the order the edits were applied.
    pub fn commit(mut self) -> Vec<Edit> {
        std::mem::take(&mut self.inverses)
    }

    /// Puts the document back now.
    pub fn rollback(self) {}
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        while let Some(inverse) = self.inverses.pop() {
            // The inverse of an edit just applied cannot fail; if it did there is nothing better to do than to
            // go on restoring the rest.
            let _ = inverse.apply(self.doc);
        }
    }
}
