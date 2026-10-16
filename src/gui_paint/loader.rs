//! Background loading of a document: `.mcpaint` directly, `.spp` through `mcsub convert` into the
//! conversion cache first.

use super::cache;
use super::job::{CancelToken, Job};
use super::mcsub::Mcsub;
use materialcraft::paint::{self, Document};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

/// A document read from disk.
pub struct Loaded {
    pub document: Document,
    /// The file the user opened (the `.spp` for imports).
    pub source: PathBuf,
    /// Importer summary line, for imports.
    pub summary: Option<String>,
}

enum Event {
    Progress(f32, String),
    Done(Result<Loaded, String>),
}

/// One load in progress. Dropping it cancels the load (and kills `mcsub`).
pub struct Loader {
    job: Job<Event>,
    source: PathBuf,
    progress: (f32, String),
}

impl Loader {
    pub fn start(source: PathBuf) -> Loader {
        let path = source.clone();
        let job = Job::spawn(move |cancel, tx| {
            let result = load(&path, cancel, tx);
            let _ = tx.send(Event::Done(result));
        });
        Loader {
            job,
            source,
            progress: (0.0, "Opening".into()),
        }
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Fraction done and the current step.
    pub fn progress(&self) -> (f32, &str) {
        (self.progress.0, &self.progress.1)
    }

    /// The result once the load has finished.
    pub fn poll(&mut self) -> Option<Result<Loaded, String>> {
        let mut done = None;
        for e in self.job.poll() {
            match e {
                Event::Progress(f, m) => self.progress = (f, m),
                Event::Done(r) => done = Some(r),
            }
        }
        if done.is_none() && self.job.is_finished() {
            done = Some(Err("The loader stopped unexpectedly.".into()));
        }
        done
    }
}

fn load(path: &Path, cancel: &CancelToken, tx: &Sender<Event>) -> Result<Loaded, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (file, summary) = match ext.as_str() {
        "mcpaint" => (path.to_path_buf(), None),
        "spp" => {
            let (file, summary) = convert(path, cancel, tx)?;
            (file, Some(summary))
        }
        _ => {
            return Err(format!(
                "{} is neither a .mcpaint document nor a .spp project.",
                path.display()
            ))
        }
    };
    let _ = tx.send(Event::Progress(0.95, "Reading document".into()));
    let document = read_document(&file)?;
    Ok(Loaded {
        document,
        source: path.to_path_buf(),
        summary,
    })
}

/// Converts a `.spp` into the cache unless an up-to-date conversion is there already.
fn convert(
    spp: &Path,
    cancel: &CancelToken,
    tx: &Sender<Event>,
) -> Result<(PathBuf, String), String> {
    let mcsub = Mcsub::locate()?;
    let out = cache::converted_path(spp, mcsub.path());
    if out.is_file() {
        return Ok((out, "Opened the cached conversion.".into()));
    }
    std::fs::create_dir_all(cache::dir())
        .map_err(|e| format!("Cannot create {}: {e}", cache::dir().display()))?;
    let part = out.with_extension("mcpaint.part");
    let progress = |f: f32, m: &str| {
        let _ = tx.send(Event::Progress(f * 0.9, m.to_string()));
    };
    let result = mcsub.convert(spp, &part, cancel, progress);
    let summary = match result {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
    };
    std::fs::rename(&part, &out)
        .map_err(|e| format!("Cannot store the converted document: {e}"))?;
    Ok((out, summary))
}

fn read_document(file: &Path) -> Result<Document, String> {
    paint::format::load_path(file).map_err(|e| format!("Cannot read {}: {e}", file.display()))
}
