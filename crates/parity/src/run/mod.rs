//! [`Runner`]: the selected subjects over a list of projects, one case per (subject, project, texture set).
//! [`survey`] answers the cheaper question of which cases could run, without baking anything.

mod case;
mod measure;
mod set_run;
pub mod survey;

pub use case::{CaseImages, CaseResult, CaseSink, Measured, Outcome};
pub use set_run::SetRun;

use crate::project::OpenedProject;
use crate::reference::References;
use crate::subject::{CaseError, Subject};
use std::path::PathBuf;

pub struct RunOptions {
    /// Stop after this many texture sets that could run (`None` = all).
    pub limit: Option<usize>,
    /// Longest side compared (references are shrunk to it, and we bake at that size).
    pub max_size: usize,
}

pub struct Runner<'a> {
    pub subjects: &'a [Box<dyn Subject>],
    pub options: RunOptions,
}

impl Runner<'_> {
    pub fn run(&self, projects: &[PathBuf], sink: &mut dyn CaseSink) -> Vec<CaseResult> {
        let mut results = Vec::new();
        let mut ran = 0usize;
        for path in projects {
            if self.options.limit.is_some_and(|l| ran >= l) {
                break;
            }
            sink.note(&format!("opening {}", path.display()));
            let project = match OpenedProject::open(path) {
                Ok(p) => p,
                Err(e) => {
                    sink.note(&format!("{}: {e}", path.display()));
                    continue;
                }
            };
            project
                .notes
                .iter()
                .for_each(|n| sink.note(&format!("{}: {n}", project.name())));
            let references = References::new(&project);
            for set in &project.sets {
                if self.options.limit.is_some_and(|l| ran >= l) {
                    break;
                }
                let (with_ref, without): (Vec<&dyn Subject>, Vec<&dyn Subject>) = self
                    .subjects
                    .iter()
                    .map(|s| s.as_ref())
                    .partition(|s| references.has(set, &s.reference()));
                let run = SetRun {
                    project: &project,
                    references: &references,
                    set,
                    max_size: self.options.max_size,
                };
                let missing = without
                    .iter()
                    .map(|s| CaseError::skipped(format!("Painter stored no {}", s.reference())));
                results.extend(run.skip_all(&without, missing, sink));
                if with_ref.is_empty() {
                    continue;
                }
                match project.high_source(set) {
                    Ok(source) => {
                        sink.note(&format!(
                            "{} / {}: running {} subject(s)",
                            project.name(),
                            set.name,
                            with_ref.len()
                        ));
                        results.extend(run.run(&with_ref, &source, sink));
                        ran += 1;
                    }
                    Err(e) => results.extend(run.skip_all(&with_ref, std::iter::repeat(e), sink)),
                }
            }
        }
        results
    }
}
