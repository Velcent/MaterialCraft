//! [`Runner`]: the selected subjects over a list of projects, one case per (subject, project, texture set).
//! [`survey`] answers the cheaper question of which cases could run, without baking anything.

mod case;
mod isolated_run;
mod measure;
mod parallel;
mod set_run;
mod stack_run;
pub mod stale;
pub mod survey;

pub use case::{CaseImages, CaseResult, CaseSink, Measured, Outcome};
pub use set_run::SetRun;

use crate::project::OpenedProject;
use crate::reference::References;
use crate::subject::{CaseError, Subject};
use materialcraft_substance::tools::parallel::CorpusPool;
use parallel::SharedSink;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct RunOptions {
    /// Stop after this many texture sets that could run (`None` = all).
    pub limit: Option<usize>,
    /// Longest side compared (references are shrunk to it, and we bake at that size).
    pub max_size: usize,
    /// Projects measured at the same time.
    pub jobs: usize,
    /// Which cases the generator and texture subjects get from the texture cache.
    pub cache_cases: CacheCases,
}

/// Cases of the subjects measured through the imported layer stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CacheCases {
    /// The channel's final look, where the effect is the only unknown (one composite).
    Final,
    /// The effect's own output read back through the rest of the stack, also next to other unknowns.
    Isolated,
    #[default]
    Both,
}

impl CacheCases {
    pub fn from_id(id: &str) -> Option<CacheCases> {
        match id {
            "final" => Some(CacheCases::Final),
            "isolated" => Some(CacheCases::Isolated),
            "both" => Some(CacheCases::Both),
            _ => None,
        }
    }
}

pub struct Runner<'a> {
    pub subjects: &'a [Box<dyn Subject>],
    pub options: RunOptions,
}

impl Runner<'_> {
    /// Every project, up to `options.jobs` at a time within the pool's memory budget. Results come back in
    /// project order, so tables and the history read the same as a run with one job; cases reach `sink` as
    /// they finish.
    pub fn run(&self, projects: &[PathBuf], sink: &mut (dyn CaseSink + Send)) -> Vec<CaseResult> {
        let shared = SharedSink::new(sink);
        let ran = AtomicUsize::new(0);
        CorpusPool::new(self.options.jobs)
            .map(
                projects,
                |path| CorpusPool::estimate(path),
                |path| self.project(path, &ran, &mut shared.handle()),
            )
            .into_iter()
            .flatten()
            .collect()
    }

    /// One project: every texture set, every subject. `ran` counts the sets that ran, across all workers.
    fn project(&self, path: &PathBuf, ran: &AtomicUsize, sink: &mut dyn CaseSink) -> Vec<CaseResult> {
        let mut results = Vec::new();
        if self.limit_reached(ran) {
            return results;
        }
        let (stack, baked): (Vec<&dyn Subject>, Vec<&dyn Subject>) = self
            .subjects
            .iter()
            .map(|s| s.as_ref())
            .partition(|s| s.generator().is_some());
        let cases = self.options.cache_cases;
        if !stack.is_empty() && cases != CacheCases::Isolated {
            results.extend(stack_run::run(&stack, path, self.options.max_size, sink));
        }
        if !stack.is_empty() && cases != CacheCases::Final {
            results.extend(isolated_run::run(&stack, path, self.options.max_size, sink));
        }
        if baked.is_empty() {
            return results;
        }
        sink.note(&format!("opening {}", path.display()));
        let project = match OpenedProject::open(path) {
            Ok(p) => p,
            Err(e) => {
                sink.note(&format!("{}: {e}", path.display()));
                return results;
            }
        };
        project
            .notes
            .iter()
            .for_each(|n| sink.note(&format!("{}: {n}", project.name())));
        let references = References::new(&project);
        for set in &project.sets {
            if self.limit_reached(ran) {
                break;
            }
            let (with_ref, without): (Vec<&dyn Subject>, Vec<&dyn Subject>) =
                baked.iter().copied().partition(|s| references.has(set, &s.reference()));
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
                    // claim a slot first, so parallel workers never run more sets than the limit
                    if !self.claim(ran) {
                        break;
                    }
                    sink.note(&format!(
                        "{} / {}: running {} subject(s)",
                        project.name(),
                        set.name,
                        with_ref.len()
                    ));
                    results.extend(run.run(&with_ref, &source, sink));
                }
                Err(e) => results.extend(run.skip_all(&with_ref, std::iter::repeat(e), sink)),
            }
        }
        results
    }

    fn limit_reached(&self, ran: &AtomicUsize) -> bool {
        self.options.limit.is_some_and(|l| ran.load(Ordering::Relaxed) >= l)
    }

    /// Counts one more set that runs; false when that would pass the limit.
    fn claim(&self, ran: &AtomicUsize) -> bool {
        let before = ran.fetch_add(1, Ordering::Relaxed);
        self.options.limit.is_none_or(|l| before < l)
    }
}
