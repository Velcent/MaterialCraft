//! [`OpenDocument`]: the loaded document, its undo history and the per-texture-set view state of the
//! Paint screen. Every change to the document goes through [`OpenDocument::edit`], so it is one step on the
//! history; undo, redo and jumps go through the history too (HI-01, HI-07).

use materialcraft::history::PaintHistory;
use materialcraft::paint::Document;
use materialcraft::paint::edit::EditError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Per-texture-set state that is not part of the document.
pub struct SetState {
    pub visible: bool,
    /// The set's maps for the current display mode are on the GPU.
    pub maps_ready: bool,
    /// The set's maps were ready at least once (its compositing warnings were logged).
    pub reported: bool,
}

/// A document shown on the Paint screen. The document itself is shared read-only with workers; an edit
/// copies it only while a worker still holds the old version.
pub struct OpenDocument {
    doc: Arc<Document>,
    /// One per open document; a new document comes with a new, empty history.
    history: PaintHistory,
    source: PathBuf,
    /// Importer summary, if the document was imported.
    pub summary: Option<String>,
    pub sets: Vec<SetState>,
}

impl OpenDocument {
    pub fn new(doc: Document, source: PathBuf, summary: Option<String>) -> OpenDocument {
        let sets = doc.texture_sets.iter().map(|_| SetState { visible: true, maps_ready: false, reported: false }).collect();
        OpenDocument { doc: Arc::new(doc), history: PaintHistory::default(), source, summary, sets }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// A handle for background workers.
    pub fn shared(&self) -> Arc<Document> {
        self.doc.clone()
    }

    pub fn history(&self) -> &PaintHistory {
        &self.history
    }

    /// Steps back once; false if there was nothing to undo. After a history move call [`Self::sync_sets`] and
    /// recomposite: the history does not say what changed.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        self.history.undo(Arc::make_mut(&mut self.doc))
    }

    /// Steps forward once; false if there was nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EditError> {
        self.history.redo(Arc::make_mut(&mut self.doc))
    }

    /// Returns the document to the state after `done` steps (the timeline's blocks).
    pub fn jump_to(&mut self, done: usize) -> Result<(), EditError> {
        self.history.jump_to(Arc::make_mut(&mut self.doc), done)
    }

    /// Keeps one view state per texture set when steps added or removed sets; returns true if the number of
    /// sets changed (the scene then needs a rebuild).
    pub fn sync_sets(&mut self) -> bool {
        let n = self.doc.texture_sets.len();
        if n == self.sets.len() {
            return false;
        }
        self.sets.resize_with(n, || SetState { visible: true, maps_ready: false, reported: false });
        true
    }

    /// Marks the maps of the listed sets outdated.
    pub fn invalidate_sets(&mut self, sets: &[usize]) {
        for i in sets {
            if let Some(s) = self.sets.get_mut(*i) {
                s.maps_ready = false;
            }
        }
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    /// File name for titles.
    pub fn name(&self) -> String {
        self.source.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
    }

    /// Visibility of each texture set, by index.
    pub fn visibility(&self) -> Vec<bool> {
        self.sets.iter().map(|s| s.visible).collect()
    }

    /// Indices of the visible texture sets.
    pub fn visible_sets(&self) -> Vec<usize> {
        self.sets.iter().enumerate().filter(|(_, s)| s.visible).map(|(i, _)| i).collect()
    }

    pub fn set_visible(&mut self, set: usize, visible: bool) {
        if let Some(s) = self.sets.get_mut(set) {
            s.visible = visible;
        }
    }

    /// Marks every set's maps as outdated (a new display mode or visibility is being computed).
    pub fn invalidate_maps(&mut self) {
        self.sets.iter_mut().for_each(|s| s.maps_ready = false);
    }

    /// Marks `set`'s maps as ready; true the first time for this document.
    pub fn mark_ready(&mut self, set: usize) -> bool {
        let Some(s) = self.sets.get_mut(set) else { return false };
        s.maps_ready = true;
        !std::mem::replace(&mut s.reported, true)
    }

    /// The index of the texture set with `id`.
    pub fn set_index(&self, id: &str) -> Option<usize> {
        self.doc.texture_sets.iter().position(|s| s.id == id)
    }
}
