//! [`ImportReview`]: the step between choosing a `.spp` and importing it. The project is analysed in the
//! background (`mcsub analyze`'s [`Analysis`]: which Substance procedurals it uses, which of them MaterialCraft
//! can replace, their measured accuracy); when something can be replaced the user picks per procedural in the
//! [`ReviewDialog`], otherwise the import starts right away.

use super::job::Job;
use super::panels::import_review::{ReviewDialog, ReviewResult};
use eframe::egui;
use materialcraft_substance::tools::accuracy::ReplacementAccuracy;
use materialcraft_substance::tools::analyze::Analysis;
use materialcraft_substance::translate::replacements::ProceduralPlan;
use std::path::{Path, PathBuf};

enum State {
    Analysing(Job<Result<Analysis, String>>),
    Choosing(ReviewDialog),
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
    state: State,
}

impl ImportReview {
    /// Starts analysing `source` in the background.
    pub fn start(source: PathBuf) -> ImportReview {
        let path = source.clone();
        let job = Job::spawn(move |_, tx| {
            let _ = tx.send(Analysis::of_project(&path, &ReplacementAccuracy::load()));
        });
        ImportReview { source, state: State::Analysing(job) }
    }

    pub fn source(&self) -> &Path {
        &self.source
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
                    Ok(analysis) if analysis.procedurals.iter().any(|p| p.replacement.is_some()) => {
                        self.state = State::Choosing(ReviewDialog::new(analysis));
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
