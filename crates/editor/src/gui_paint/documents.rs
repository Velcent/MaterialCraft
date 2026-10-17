//! Opening, replacing and closing the document of the Paint screen.

use super::document_state::OpenDocument;
use super::loader::{Loaded, Loader};
use super::{PaintMode, Pick};
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::diag_entries;
use std::path::PathBuf;

impl PaintMode {
    /// Shows a file dialog and opens the chosen file. Returns false if the user cancelled.
    pub fn pick(&mut self, pick: Pick) -> bool {
        let dialog = match pick {
            Pick::Any => rfd::FileDialog::new()
                .add_filter("Paint document or Substance Painter project", &["mcpaint", "spp"])
                .add_filter("MaterialCraft paint document", &["mcpaint"]),
            Pick::Substance => rfd::FileDialog::new().add_filter("Substance 3D Painter project", &["spp"]),
        };
        match dialog.pick_file() {
            Some(p) => {
                self.open(p);
                true
            }
            None => false,
        }
    }

    /// Starts loading `path` in the background (the current document stays until it is done).
    pub fn open(&mut self, path: PathBuf) {
        self.diag.info(Area::Project, format!("Opening {}", path.display()));
        self.loader = Some(Loader::start(path));
    }

    pub(super) fn poll_loader(&mut self) {
        let Some(result) = self.loader.as_mut().and_then(|l| l.poll()) else {
            return;
        };
        let source = self.loader.take().map(|l| l.source().display().to_string()).unwrap_or_default();
        match result {
            Ok(loaded) => self.show(loaded),
            Err(e) => {
                self.diag.error(area_of_load(&source), format!("{source}: {e}"));
                if let Some(shot) = &self.shot {
                    shot.abort(&format!("{source}: {e}"));
                }
            }
        }
    }

    /// Replaces the current document with `loaded`.
    pub(super) fn show(&mut self, loaded: Loaded) {
        let mut doc = OpenDocument::new(loaded.document, loaded.source, loaded.summary);
        let d = doc.doc();
        self.diag.info(Area::Project, format!("Opened {}: {} texture sets, {} mesh parts.", doc.source().display(), d.texture_sets.len(), d.mesh.parts.len()));
        if let Some(report) = d.import_report.as_ref().filter(|_| doc.summary.is_some()) {
            // The import report panel keeps the details; the log gets its summary line (LG-12).
            self.diag.emit(diag_entries::import_summary(report, &doc.source().display().to_string(), doc.summary.as_deref()));
        }
        self.layers.reset();
        self.scene.rebuild(&mut doc, self.display);
        self.viewport.reset_camera();
        self.document = Some(doc);
    }

    pub(super) fn close(&mut self) {
        self.scene.clear(self.viewport.gpu());
        self.layers.reset();
        self.document = None;
        self.diag.info(Area::Project, "Document closed.");
    }
}

/// Failed loads of Substance projects are import problems; anything else is a project problem.
fn area_of_load(source: &str) -> Area {
    if source.to_ascii_lowercase().ends_with(".spp") { Area::Import } else { Area::Project }
}
