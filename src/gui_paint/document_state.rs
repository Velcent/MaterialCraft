//! [`OpenDocument`]: the loaded document plus the per-texture-set view state of the Paint screen.

use materialcraft::paint::Document;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Per-texture-set state that is not part of the document.
pub struct SetState {
    pub visible: bool,
    /// The set's maps for the current display mode are on the GPU.
    pub maps_ready: bool,
}

/// A document shown on the Paint screen. The document itself is shared read-only with workers.
pub struct OpenDocument {
    doc: Arc<Document>,
    source: PathBuf,
    /// Importer summary, if the document was imported.
    pub summary: Option<String>,
    pub sets: Vec<SetState>,
}

impl OpenDocument {
    pub fn new(doc: Document, source: PathBuf, summary: Option<String>) -> OpenDocument {
        let sets = doc
            .texture_sets
            .iter()
            .map(|_| SetState {
                visible: true,
                maps_ready: false,
            })
            .collect();
        OpenDocument {
            doc: Arc::new(doc),
            source,
            summary,
            sets,
        }
    }

    pub fn doc(&self) -> &Document {
        &self.doc
    }

    /// A handle for background workers.
    pub fn shared(&self) -> Arc<Document> {
        self.doc.clone()
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    /// File name for titles.
    pub fn name(&self) -> String {
        self.source
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    /// Visibility of each texture set, by index.
    pub fn visibility(&self) -> Vec<bool> {
        self.sets.iter().map(|s| s.visible).collect()
    }

    /// Indices of the visible texture sets.
    pub fn visible_sets(&self) -> Vec<usize> {
        self.sets
            .iter()
            .enumerate()
            .filter(|(_, s)| s.visible)
            .map(|(i, _)| i)
            .collect()
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

    pub fn mark_ready(&mut self, set: usize) {
        if let Some(s) = self.sets.get_mut(set) {
            s.maps_ready = true;
        }
    }
}
