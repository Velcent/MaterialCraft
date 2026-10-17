//! [`ImportReview`]: the step between choosing a `.spp` and importing it. The project is analysed in the
//! background (`mcsub analyze`'s [`Analysis`]: which Substance procedurals it uses, which of them MaterialCraft
//! can replace, their measured accuracy; and its Substance material fills with the tray materials suggested
//! for them, [`MaterialMatches`]); when something can be replaced the user picks in the [`ReviewDialog`],
//! otherwise the import starts right away.

use super::job::Job;
use super::panels::import_review::{ReviewDialog, ReviewResult};
use eframe::egui;
use materialcraft_substance::tools::accuracy::ReplacementAccuracy;
use materialcraft_substance::tools::analyze::Analysis;
use materialcraft_substance::tools::material_match::{MaterialMatches, TrayMaterials};
use materialcraft_substance::translate::replacements::ProceduralPlan;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the background analysis found.
pub struct Findings {
    pub analysis: Analysis,
    pub materials: MaterialMatches,
    pub tray: Arc<TrayMaterials>,
}

enum State {
    Analysing(Job<Result<Findings, String>>),
    Choosing(Box<ReviewDialog>),
}

/// What the review decided this frame.
pub enum ReviewOutcome {
    /// Still analysing or waiting for the user.
    Pending,
    /// Import with these choices.
    Import(ProceduralPlan),
    Cancelled,
}

pub struct ImportReview {
    source: PathBuf,
    /// The asset library folders whose materials are offered (and embedded on import).
    library: Vec<PathBuf>,
    state: State,
}

impl ImportReview {
    /// Starts analysing `source` in the background, offering the materials of the `library` folders.
    pub fn start(source: PathBuf, library: Vec<PathBuf>) -> ImportReview {
        let (path, folders) = (source.clone(), library.clone());
        let job = Job::spawn(move |_, tx| {
            let _ = tx.send(analyse(&path, &folders));
        });
        ImportReview { source, library, state: State::Analysing(job) }
    }

    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn library(&self) -> &[PathBuf] {
        &self.library
    }

    /// Polls the analysis and shows the dialog once it is there.
    pub fn ui(&mut self, ctx: &egui::Context) -> ReviewOutcome {
        match &mut self.state {
            State::Analysing(job) => {
                let Some(result) = job.poll().into_iter().next() else {
                    if job.is_finished() {
                        // The analysis thread ended without an answer: import as before replacements existed.
                        return ReviewOutcome::Import(ProceduralPlan::default());
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                    return ReviewOutcome::Pending;
                };
                match result {
                    Ok(f) if f.analysis.procedurals.iter().any(|p| p.replacement.is_some()) || !f.materials.materials.is_empty() => {
                        self.state = State::Choosing(Box::new(ReviewDialog::new(f)));
                        ReviewOutcome::Pending
                    }
                    // Nothing to choose (or the layer document is unreadable, which the importer reports).
                    _ => ReviewOutcome::Import(ProceduralPlan::default()),
                }
            }
            State::Choosing(dialog) => match dialog.show(ctx, &self.source) {
                Some(ReviewResult::Import(plan)) => ReviewOutcome::Import(plan),
                Some(ReviewResult::Cancel) => ReviewOutcome::Cancelled,
                None => ReviewOutcome::Pending,
            },
        }
    }
}

/// The analysis, and the tray materials ranked for each material fill (Painter's look measured in the project).
fn analyse(path: &Path, library: &[PathBuf]) -> Result<Findings, String> {
    let analysis = Analysis::of_project(path, &ReplacementAccuracy::load())?;
    let tray = Arc::new(match analysis.materials().next() {
        Some(_) => TrayMaterials::load(library),
        None => TrayMaterials::default(),
    });
    let materials = MaterialMatches::measure(path, &analysis, &tray)?;
    Ok(Findings { analysis, materials, tray })
}
