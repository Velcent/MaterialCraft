//! [`LibraryImports`]: the project assets the import review ticked (roadmap step 46), written into the user's library
//! in the background by the importer's [`LibraryExport`] while the import runs. Each written item is logged; the tray
//! picks the files up on its next poll. The imported document is not involved.

use super::PaintMode;
use super::import_review::LibraryRequest;
use super::job::Job;
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};
use materialcraft_substance::project::SppProject;
use materialcraft_substance::translate::layers::images::ProjectImages;
use materialcraft_substance::translate::library::{LibraryExport, Written};
use std::path::PathBuf;

/// The running writes.
#[derive(Default)]
pub struct LibraryImports {
    jobs: Vec<Job<Written>>,
}

impl LibraryImports {
    /// Starts writing `request`'s items from the project at `source`.
    pub fn start(&mut self, source: PathBuf, request: LibraryRequest) {
        self.jobs.push(Job::spawn(move |_, tx| {
            let project = match SppProject::open(&source) {
                Ok(p) => p,
                Err(e) => {
                    let _ = tx.send(Written { key: String::new(), name: source.display().to_string(), result: Err(e.to_string()) });
                    return;
                }
            };
            let images = ProjectImages::new(&project);
            for w in LibraryExport::new(&request.candidates, &images).write(&request.keys, &request.library) {
                let _ = tx.send(w);
            }
        }));
    }

    /// What was written since the last call; finished writes are dropped.
    pub fn poll(&mut self) -> Vec<Written> {
        let out = self.jobs.iter_mut().flat_map(|j| j.poll()).collect();
        self.jobs.retain(|j| !j.is_finished());
        out
    }

    pub fn is_running(&self) -> bool {
        !self.jobs.is_empty()
    }
}

impl PaintMode {
    /// Logs what the library writes finished.
    pub(super) fn poll_library_imports(&mut self, ctx: &egui::Context) {
        for w in self.library_imports.poll() {
            match w.result {
                Ok(saved) => self.diag.info(Area::Assets, format!("Added \"{}\" to your library ({}).", w.name, saved.reference)),
                Err(e) => self.diag.warn(Area::Assets, format!("\"{}\" was not added to your library: {e}", w.name)),
            }
        }
        if self.library_imports.is_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}
