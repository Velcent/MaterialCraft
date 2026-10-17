//! Reports of a run: the terminal table ([`text`]) and the HTML page with images in the run's cache folder
//! ([`HtmlReport`], [`html`], [`images`], [`run_dir`]).

pub mod html;
pub mod images;
pub mod run_dir;
pub mod summary;
pub mod text;

pub use run_dir::RunDir;

use crate::run::{CaseImages, CaseResult, CaseSink};
use html::{Entry, RunInfo};
use std::path::PathBuf;

/// Collects cases into the run folder as they finish (images right away, the page at the end) and echoes
/// progress to stderr.
pub struct HtmlReport {
    dir: RunDir,
    entries: Vec<Entry>,
    /// Image-writing failures (the case still counts; the page shows no images for it).
    pub problems: Vec<String>,
}

impl HtmlReport {
    pub fn new(dir: RunDir) -> HtmlReport {
        HtmlReport {
            dir,
            entries: Vec::new(),
            problems: Vec::new(),
        }
    }

    pub fn dir(&self) -> &RunDir {
        &self.dir
    }

    /// Writes `index.html`; returns its path.
    pub fn finish(&self, time: &str, commit: &str, projects: usize) -> std::io::Result<PathBuf> {
        let info = RunInfo {
            id: &self.dir.id,
            time,
            commit,
            projects,
        };
        let path = self.dir.path.join("index.html");
        std::fs::write(&path, html::page(&info, &self.entries))?;
        Ok(path)
    }
}

impl CaseSink for HtmlReport {
    fn case(&mut self, result: &CaseResult, images: Option<&CaseImages>) {
        let files = images.and_then(|img| {
            let stem = images::stem(self.entries.len(), &[result.subject, &result.project_name, &result.set]);
            images::write_case(&self.dir.path, &stem, img)
                .map_err(|e| {
                    self.problems
                        .push(format!("{}: images not written: {e}", result.label()))
                })
                .ok()
        });
        if let Some(m) = result.measured() {
            eprintln!(
                "  {:<20} {:<40} MAE {:>6.2}  ≤2/255 {:>5.1} %",
                result.subject,
                result.label(),
                m.metrics.mae_steps(),
                m.metrics.overall.within
            );
        }
        self.entries.push(Entry {
            result: result.clone(),
            files,
        });
    }

    fn note(&mut self, message: &str) {
        eprintln!("{message}");
    }
}
