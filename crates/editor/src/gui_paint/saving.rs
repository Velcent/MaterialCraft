//! File > Save and Save As on the Paint screen. A save embeds the library assets the document uses (and drops the
//! ones it no longer uses), then writes a copy of the document on a background thread through the format's
//! temporary-file-and-rename path, so a failed save never damages the file and the screen stays responsive for
//! documents of a gigabyte. The status bar shows the bytes written. A document without a `.mcpaint` file (an
//! imported `.spp`, the demo) is saved with Save As, suggesting a name next to its source.

use super::PaintMode;
use super::document_state::OpenDocument;
use super::job::Job;
use super::unsaved::Then;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::material_fill::FolderResolver;
use materialcraft::paint::embedded::EmbedReport;
use materialcraft::paint::{Document, format};
use std::path::{Path, PathBuf};

/// Progress is sent at most once per this many bytes.
const PROGRESS_STEP: u64 = 4 << 20;

enum Event {
    Written(u64),
    Done(Result<(), String>),
}

/// One save in progress.
pub struct SaveJob {
    job: Job<Event>,
    path: PathBuf,
    written: u64,
    /// The size of the file being replaced, as a guess of the total.
    estimate: Option<u64>,
    /// What to do once the save succeeded (close, open another, quit).
    then: Option<Then>,
}

impl SaveJob {
    /// Writes `doc` to `path` on a new thread.
    pub fn start(doc: Document, path: PathBuf, then: Option<Then>) -> SaveJob {
        let estimate = std::fs::metadata(&path).ok().map(|m| m.len()).filter(|n| *n > 0);
        let target = path.clone();
        let job = Job::spawn(move |_, tx| {
            let mut sent = 0u64;
            let result = format::save_path_observed(&doc, &target, &mut |n| {
                if n >= sent + PROGRESS_STEP {
                    sent = n;
                    let _ = tx.send(Event::Written(n));
                }
            });
            let _ = tx.send(Event::Done(result.map_err(|e| e.to_string())));
        });
        SaveJob { job, path, written: 0, estimate, then }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// (bytes written, the expected total if known) for the status bar.
    pub fn progress(&self) -> (u64, Option<u64>) {
        (self.written, self.estimate)
    }

    /// Runs `then` once this save succeeded (instead of what was waiting before).
    pub fn then_after(&mut self, then: Then) {
        self.then = Some(then);
    }

    /// Blocks until the save has finished (on exit).
    pub fn wait(mut self) -> Result<(), String> {
        loop {
            if let Some(r) = self.poll() {
                return r;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// The result once the save has finished.
    fn poll(&mut self) -> Option<Result<(), String>> {
        let mut done = None;
        for e in self.job.poll() {
            match e {
                Event::Written(n) => self.written = n,
                Event::Done(r) => done = Some(r),
            }
        }
        if done.is_none() && self.job.is_finished() {
            done = Some(Err("the save stopped unexpectedly".into()));
        }
        done
    }
}

/// Where Save As suggests saving a document opened from `source`: the same folder and name as a `.mcpaint`.
pub fn suggested_path(source: &Path) -> PathBuf {
    let stem = source.file_stem().map_or_else(|| "Untitled".into(), |s| s.to_string_lossy().into_owned());
    let stem = if stem.is_empty() { "Untitled".to_string() } else { stem };
    source.with_file_name(format!("{stem}.{EXTENSION}"))
}

/// `path` ending in `.mcpaint` (file dialogs may return a name without it).
pub fn with_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION)) {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".");
    name.push(EXTENSION);
    PathBuf::from(name)
}

const EXTENSION: &str = "mcpaint";

/// Gets `doc` ready to be written: embeds the library assets its layers use (read with `lookup`), drops embedded
/// ones nothing uses any more (`paint::embedded::embed_used`) and returns the copy to write.
pub fn prepare(doc: &mut OpenDocument, lookup: impl FnMut(&str) -> Option<Vec<u8>>) -> (Document, EmbedReport) {
    let report = doc.embed_assets(lookup);
    (doc.begin_save(), report)
}

impl PaintMode {
    /// File > Save: writes the document to its file, or asks where (Save As) if it has none. `then` runs once the
    /// save succeeded.
    pub(super) fn save(&mut self, then: Option<Then>) {
        let Some(doc) = &self.document else { return };
        match doc.file().map(Path::to_path_buf) {
            Some(path) => self.write_document(path, then),
            None => self.save_as(then),
        }
    }

    /// File > Save As: asks for a `.mcpaint` file and writes the document there.
    pub(super) fn save_as(&mut self, then: Option<Then>) {
        let Some(doc) = &self.document else { return };
        let suggested = suggested_path(doc.file().unwrap_or(doc.source()));
        let mut dialog = rfd::FileDialog::new().add_filter("MaterialCraft paint document", &[EXTENSION]);
        if let Some(dir) = suggested.parent().filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = suggested.file_name() {
            dialog = dialog.set_file_name(name.to_string_lossy());
        }
        match dialog.save_file() {
            Some(path) => self.write_document(with_extension(path), then),
            None => self.diag.info(Area::Project, "Not saved."),
        }
    }

    /// Embeds the assets the document uses and starts writing it to `path`.
    fn write_document(&mut self, path: PathBuf, then: Option<Then>) {
        if self.saving.job.is_some() {
            self.diag.info(Area::Project, "A save is already running.");
            return;
        }
        let lookup = self.asset_lookup();
        let Some(doc) = &mut self.document else { return };
        let (copy, report) = prepare(doc, lookup);
        if !report.missing.is_empty() {
            self.diag.warn(Area::Assets, format!("{} could not be found in the libraries; the document is saved without them.", report.missing.join(", ")));
        }
        self.diag.info(Area::Project, format!("Saving {}", path.display()));
        self.saving.job = Some(SaveJob::start(copy, path, then));
    }

    /// Finishes a save once its thread is done; then runs what was waiting for it.
    pub(super) fn poll_save(&mut self) {
        let Some(result) = self.saving.job.as_mut().and_then(SaveJob::poll) else { return };
        let Some(job) = self.saving.job.take() else { return };
        let Some(doc) = &mut self.document else { return };
        match result {
            Ok(()) => {
                doc.saved_to(job.path.clone());
                self.diag.info(Area::Project, format!("Saved {}.", job.path.display()));
                // Asks again if the document was changed while it was being saved.
                if let Some(then) = job.then {
                    self.guard(then);
                }
            }
            Err(e) => {
                doc.save_failed();
                self.diag.error(Area::Project, format!("{} was not saved: {e}", job.path.display()));
            }
        }
    }

    /// Reads a library reference (`materials/rust.mca`) from the document's folder or the library roots: what a save
    /// embeds.
    pub(super) fn asset_lookup(&self) -> Box<dyn FnMut(&str) -> Option<Vec<u8>>> {
        let dir = self.document.as_ref().and_then(|d| d.source().parent().map(|p| p.to_path_buf()));
        let folders = FolderResolver::new(dir.into_iter().chain(self.tray.material_folders()));
        Box::new(move |r| std::fs::read(folders.locate(r)?).ok())
    }

    /// True while a save is running.
    pub(super) fn is_saving(&self) -> bool {
        self.saving.job.is_some()
    }
}

#[cfg(test)]
#[path = "saving_tests.rs"]
mod save_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_sit_next_to_the_source() {
        assert_eq!(suggested_path(Path::new("/p/Robot.spp")), PathBuf::from("/p/Robot.mcpaint"));
        assert_eq!(suggested_path(Path::new("/p/Robot.mcpaint")), PathBuf::from("/p/Robot.mcpaint"));
        assert_eq!(suggested_path(Path::new("demo")), PathBuf::from("demo.mcpaint"));
        assert_eq!(with_extension(PathBuf::from("/p/a")), PathBuf::from("/p/a.mcpaint"));
        assert_eq!(with_extension(PathBuf::from("/p/a.MCPAINT")), PathBuf::from("/p/a.MCPAINT"));
        assert_eq!(with_extension(PathBuf::from("/p/a.v2")), PathBuf::from("/p/a.v2.mcpaint"));
    }
}
