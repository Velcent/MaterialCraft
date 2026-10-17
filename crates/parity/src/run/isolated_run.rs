//! The generator and texture subjects on one project, isolated: each case is one replaced effect in one
//! channel, Painter's output of that effect read back from the texture cache through the rest of the stack
//! (`materialcraft_substance::tools::verify_layers::isolate`) and ours read back the same way. The compared
//! images are the effect's own output (a mask's grey value, a fill's channel value), on the texels where the
//! read-back is well conditioned and no other unknown layer reaches.

use super::case::{CaseImages, CaseResult, CaseSink, Measured, Outcome};
use crate::metrics::Metrics;
use crate::subject::{GRAY, RGB, Subject};
use materialcraft_substance::tools::verify_layers::isolate::{CasePlan, IsolateOptions, IsolatedCase, isolated_cases};
use std::path::Path;
use std::time::Instant;

/// Cases with fewer compared texels than this are reported as skipped (too little to say anything).
pub const MIN_TEXELS: usize = 256;

/// Every isolated case of `subjects` (generator and texture subjects) in the project at `path`.
pub fn run(subjects: &[&dyn Subject], path: &Path, max_size: usize, sink: &mut dyn CaseSink) -> Vec<CaseResult> {
    let started = Instant::now();
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    sink.note(&format!("{name}: isolated cases"));
    let subject_of = |plan: &CasePlan| {
        let ours = plan.candidate.ours?;
        subjects.iter().find(|s| s.generator() == Some(ours)).copied()
    };
    let options = IsolateOptions {
        max_size,
        ..IsolateOptions::default()
    };
    let mut out = Vec::new();
    let mut each = |case: IsolatedCase| {
        let Some(subject) = subject_of(&case.plan) else {
            return;
        };
        let seconds = started.elapsed().as_secs_f64();
        out.push(measure(subject, path, &name, case, seconds, sink));
    };
    if let Err(e) = isolated_cases(path, &|p| subject_of(p).is_some(), &options, &mut each) {
        sink.note(&format!("{name}: {e}"));
    }
    out
}

fn measure(
    subject: &dyn Subject,
    path: &Path,
    name: &str,
    case: IsolatedCase,
    seconds: f64,
    sink: &mut dyn CaseSink,
) -> CaseResult {
    let m = &case.measurement;
    let s = m.stats;
    let set = format!(
        "{} / {} · {} (isolated)",
        case.set, case.channel, case.plan.candidate.path
    );
    let result = |outcome| CaseResult {
        subject: subject.id(),
        project: path.to_path_buf(),
        project_name: name.to_string(),
        set: set.clone(),
        outcome,
    };
    let Some(ours) = &m.ours else {
        let why = m
            .notes
            .first()
            .cloned()
            .unwrap_or_else(|| "our replacement was not composited".into());
        let r = result(Outcome::Skipped(why));
        sink.case(&r, None);
        return r;
    };
    if s.solved < MIN_TEXELS {
        let r = result(Outcome::Skipped(format!(
            "only {} texels can be read back ({} on the mesh, {} clear of other unknowns)",
            s.solved, s.island, s.clean
        )));
        sink.case(&r, None);
        return r;
    }
    let components = if m.components == 3 { RGB } else { GRAY };
    let (metrics, errors) = Metrics::measure(ours, &m.theirs, components, &m.mask);
    let mut notes = vec![
        format!(
            "the effect's own output, read back through {} composites: {} of {} texels on the mesh compared ({} clear of {} other unknown part(s))",
            s.composites,
            s.solved,
            s.island,
            s.clean,
            case.plan.nuisances.len()
        ),
        format!(
            "{:.1} % of the compared texels lay outside what the stack can give (clamped)",
            percent(s.outside, s.solved)
        ),
    ];
    if case.plan.candidate.projection != 0 {
        notes.push(format!(
            "Painter projects this fill with mode {}; ours is laid on in UV",
            case.plan.candidate.projection
        ));
    }
    notes.extend(m.notes.iter().cloned());
    let measured = Measured {
        metrics,
        size: (m.theirs.w, m.theirs.h),
        reference: format!("texture cache {} / {}, read back", case.set, case.channel),
        reference_size: (m.theirs.w, m.theirs.h),
        high: "–".into(),
        notes,
        unmapped: Vec::new(),
        settings_hash: String::new(),
        stale: None,
        seconds,
    };
    let r = result(Outcome::Measured(Box::new(measured)));
    sink.case(
        &r,
        Some(&CaseImages {
            ours,
            theirs: &m.theirs,
            mask: &m.mask,
            errors: &errors,
        }),
    );
    r
}

fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}
