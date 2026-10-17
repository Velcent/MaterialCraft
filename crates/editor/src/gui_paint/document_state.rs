//! [`OpenDocument`]: the loaded document, its undo history and the per-texture-set view state of the
//! Paint screen. Every change to the document goes through [`OpenDocument::edit`], so it is one step on the
//! history; undo, redo and jumps go through the history too (HI-01, HI-07).

use super::save_point::SavePoint;
use materialcraft::history::{CoalesceKey, PaintHistory};
use materialcraft::paint::edit::{EditError, Transaction};
use materialcraft::paint::embedded::{EmbedReport, embed_used};
use materialcraft::paint::{Document, TextureSet};
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

/// A document shown on the Paint screen. Background workers get views of it ([`Self::maps_view`],
/// [`Self::mesh_view`]), never the document itself, so an edit never has to copy it.
pub struct OpenDocument {
    doc: Document,
    /// One per open document; a new document comes with a new, empty history.
    history: PaintHistory,
    source: PathBuf,
    /// The `.mcpaint` file Save writes to: the opened document or the last Save As; `None` for imports and the demo
    /// (they are saved with Save As).
    file: Option<PathBuf>,
    /// The state last saved (or opened); the document is dirty when the history is elsewhere.
    saved: SavePoint,
    /// The state a running save writes, which becomes `saved` when it succeeds.
    saving: Option<SavePoint>,
    /// Importer summary, if the document was imported.
    pub summary: Option<String>,
    pub sets: Vec<SetState>,
}

impl OpenDocument {
    pub fn new(doc: Document, source: PathBuf, summary: Option<String>) -> OpenDocument {
        let sets = doc.texture_sets.iter().map(|_| SetState { visible: true, maps_ready: false, reported: false }).collect();
        let history = PaintHistory::default();
        let file = source.extension().is_some_and(|e| e.eq_ignore_ascii_case("mcpaint")).then(|| source.clone());
        let saved = SavePoint::of(&history);
        OpenDocument { doc, history, source, file, saved, saving: None, summary, sets }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// What a maps worker compositing `sets` reads: those texture sets and the output settings. Other sets keep
    /// only their placement; the mesh, the kept files and the import report are left out. Cheap (layer images
    /// share their tiles), and the document itself stays unshared, so edits while the worker runs change it in
    /// place instead of copying it.
    pub fn maps_view(&self, sets: &[usize]) -> Arc<Document> {
        let doc = &self.doc;
        let texture_sets: Vec<TextureSet> =
            doc.texture_sets.iter().enumerate().map(|(i, s)| if sets.contains(&i) { s.clone() } else { placement(s) }).collect();
        // polygon fills composite through the set's surface map, which needs the mesh
        let mesh = if texture_sets.iter().any(TextureSet::uses_polygon_fill) { doc.mesh.clone() } else { Default::default() };
        Arc::new(Document { texture_sets, mesh, height_filter: doc.height_filter, ..Document::default() })
    }

    /// What a geometry worker reads: the mesh and each set's slot and UV set (no layers, baked maps or kept
    /// files). Copies the mesh once per geometry build, which is rare (a new document, changed visibility).
    pub fn mesh_view(&self) -> Arc<Document> {
        let doc = &self.doc;
        let sets = doc.texture_sets.iter().map(placement).collect();
        Arc::new(Document { mesh: doc.mesh.clone(), texture_sets: sets, ..Document::default() })
    }

    /// What a bake queue reads besides its jobs: the mesh, copied once per queue (the jobs carry the settings
    /// and the results come back as edits, so no texture set is needed).
    pub fn bake_mesh(&self) -> Arc<materialcraft::mesh::Mesh> {
        Arc::new(self.doc.mesh.clone())
    }

    pub fn history(&self) -> &PaintHistory {
        &self.history
    }

    /// Runs `op` on the document as one history step named `label`. Edits with the same `key` join the
    /// open step (one slider drag); an operation that makes no edit records nothing. On error the document
    /// is unchanged.
    pub fn edit<T>(&mut self, label: &str, key: Option<CoalesceKey>, op: impl FnOnce(&mut Transaction) -> Result<T, EditError>) -> Result<T, EditError> {
        let mut tx = Transaction::new(&mut self.doc);
        let out = op(&mut tx)?;
        let inverses = tx.commit();
        if !inverses.is_empty() {
            let at = self.at_save_points();
            self.history.record_all(&self.doc, label, key, inverses);
            self.recorded(at);
        }
        Ok(out)
    }

    /// The document for a brush stroke, which paints into its images directly and hands the inverses of what it
    /// did to [`Self::record_stroke`] at pen up (one step per stroke, PT-54). Nothing else may use this.
    pub fn doc_for_strokes(&mut self) -> &mut Document {
        &mut self.doc
    }

    /// Records a finished stroke's inverses (in the order its edits were made) as one closed history step.
    pub fn record_stroke(&mut self, label: &str, inverses: Vec<materialcraft::paint::edit::Edit>) {
        if !inverses.is_empty() {
            let at = self.at_save_points();
            self.history.record_all(&self.doc, label, None, inverses);
            self.recorded(at);
        }
        self.history.close();
    }

    /// Whether the history is at the saved state and at the state being saved, before a step is recorded.
    fn at_save_points(&self) -> (bool, bool) {
        (self.saved.is_current(&self.history), self.saving.is_some_and(|p| p.is_current(&self.history)))
    }

    /// After a step was recorded: it names the save points it leads away from.
    fn recorded(&mut self, (saved, saving): (bool, bool)) {
        self.saved.note_recorded(saved, &self.history);
        if let Some(p) = &mut self.saving {
            p.note_recorded(saving, &self.history);
        }
    }

    /// True if the document has changes that are not in its file (the history is not at the saved state).
    pub fn is_dirty(&self) -> bool {
        !self.saved.is_current(&self.history)
    }

    /// The `.mcpaint` file Save writes to; `None` if the document has none yet (an import): use Save As.
    pub fn file(&self) -> Option<&Path> {
        self.file.as_deref()
    }

    /// Before a save: embeds the library assets the layers use and drops embedded ones nothing uses any more
    /// (`lookup` reads a library reference). Not a history step: embedding changes no pixel, and an undo that
    /// brings back a reference finds the asset in the library again or embeds it on the next save.
    pub fn embed_assets(&mut self, lookup: impl FnMut(&str) -> Option<Vec<u8>>) -> EmbedReport {
        embed_used(&mut self.doc, lookup)
    }

    /// Starts a save of the current state: ends the open step (a later edit must not join the saved one) and
    /// returns the document to write (a cheap copy: images share their tiles).
    pub fn begin_save(&mut self) -> Document {
        self.history.close();
        self.saving = Some(SavePoint::of(&self.history));
        self.doc.clone()
    }

    /// The save started by [`Self::begin_save`] wrote `path`: that state is the saved one, and `path` the file.
    pub fn saved_to(&mut self, path: PathBuf) {
        if let Some(p) = self.saving.take() {
            self.saved = p;
        }
        self.source = path.clone();
        self.file = Some(path);
    }

    /// The save started by [`Self::begin_save`] failed: nothing changes.
    pub fn save_failed(&mut self) {
        self.saving = None;
    }

    /// Ends the open step (the end of a slider drag).
    pub fn close_step(&mut self) {
        self.history.close();
    }

    /// Steps back once; false if there was nothing to undo. After a history move call [`Self::sync_sets`] and
    /// recomposite: the history does not say what changed.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        self.history.undo(&mut self.doc)
    }

    /// Steps forward once; false if there was nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EditError> {
        self.history.redo(&mut self.doc)
    }

    /// Returns the document to the state after `done` steps (the timeline's blocks).
    pub fn jump_to(&mut self, done: usize) -> Result<(), EditError> {
        self.history.jump_to(&mut self.doc, done)
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

    /// The name with a marker while there are unsaved changes, for the window title and the menu bar.
    pub fn title(&self) -> String {
        if self.is_dirty() { format!("{} \u{25CF}", self.name()) } else { self.name() }
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

/// A set's placement on the mesh without its layers and baked maps.
fn placement(set: &TextureSet) -> TextureSet {
    let mut out = TextureSet::new(set.id.clone(), set.name.clone(), set.resolution);
    out.material_slot = set.material_slot;
    out.uv_set = set.uv_set;
    out.channels = set.channels.clone();
    out
}

#[cfg(test)]
#[path = "document_state_tests.rs"]
mod tests;
