//! Pins the baker's parity on a few of the user's projects: every baked subject's median MAE (stale references left
//! out) must stay at or below what it reached when these numbers were set. Skipped unless `MC_SUBSTANCE_CORPUS`
//! names the folder holding the projects (searched recursively); nothing read from them is kept.
//!
//! Raise a pin only with a reason in the parity README; lower it when the baker gets closer.

use materialcraft_parity::corpus::ProjectList;
use materialcraft_parity::report::summary::SubjectSummary;
use materialcraft_parity::run::{CacheCases, CaseImages, CaseResult, CaseSink, RunOptions, Runner};
use materialcraft_parity::subjects;
use std::path::PathBuf;

/// Projects measured (file names below the corpus folder).
const PROJECTS: &[&str] = &["VASE.spp", "GLUG.spp"];
/// Largest median MAE (8-bit steps) per subject over those projects' three sets, compared at 512 (set 2026-10-18).
const PINNED: &[(&str, f64)] = &[
    ("baked.normal", 0.5),
    ("baked.world_normal", 0.5),
    ("baked.position", 0.5),
    ("baked.ao", 8.0),
    ("baked.curvature", 2.5),
    ("baked.thickness", 20.5),
    ("baked.id", 0.5),
];
/// Room for ray noise between runs.
const SLACK: f64 = 0.5;

struct Quiet;

impl CaseSink for Quiet {
    fn case(&mut self, _: &CaseResult, _: Option<&CaseImages>) {}
}

fn projects() -> Option<Vec<PathBuf>> {
    let root = std::env::var_os("MC_SUBSTANCE_CORPUS")?;
    let all = ProjectList {
        projects: Vec::new(),
        corpora: vec![PathBuf::from(root)],
    }
    .resolve()
    .ok()?;
    let picked: Vec<PathBuf> = all
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| PROJECTS.iter().any(|w| n.to_string_lossy() == *w))
        })
        .collect();
    (!picked.is_empty()).then_some(picked)
}

#[test]
fn baked_medians_stay_pinned() {
    let Some(projects) = projects() else { return };
    let chosen = subjects::select("baked");
    let runner = Runner {
        subjects: &chosen,
        options: RunOptions {
            limit: None,
            max_size: 512,
            jobs: 2,
            cache_cases: CacheCases::default(),
        },
    };
    let results = runner.run(&projects, &mut Quiet);
    let mut worse = Vec::new();
    for summary in SubjectSummary::of(&results) {
        let Some(&(_, pin)) = PINNED.iter().find(|(s, _)| *s == summary.subject) else {
            continue;
        };
        if let Some(median) = summary.mae_median
            && median > pin + SLACK
        {
            worse.push(format!("{}: median {median:.2} > pinned {pin:.2}", summary.subject));
        }
    }
    assert!(
        worse.is_empty(),
        "baked parity got worse on {PROJECTS:?}:\n{}",
        worse.join("\n")
    );
}
