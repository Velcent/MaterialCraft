//! The generator subjects on one project: the importer's generator cases (channels where only one replaced
//! procedural keeps the import from being exact), each measured as a case of the subject of that generator,
//! with our composite and Painter's cache shrunk to the compared size.

use super::case::{CaseImages, CaseResult, CaseSink, Measured, Outcome};
use crate::image_ops::resized;
use crate::metrics::Metrics;
use crate::subject::{GRAY, RGB, Subject};
use materialcraft_substance::tools::verify_layers::generator_cases::{GeneratorCase, generator_cases};
use std::path::Path;
use std::time::Instant;

/// Every case of `subjects` (all generator subjects) in the project at `path`.
pub fn run(subjects: &[&dyn Subject], path: &Path, max_size: usize, sink: &mut dyn CaseSink) -> Vec<CaseResult> {
    let started = Instant::now();
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    sink.note(&format!("{name}: generator cases"));
    let subject_of = |generators: &[String]| match generators {
        [one] => subjects.iter().find(|s| s.generator() == Some(one.as_str())).copied(),
        _ => None,
    };
    let cases = match generator_cases(path, &|g| subject_of(g).is_some()) {
        Ok(c) => c,
        Err(e) => {
            sink.note(&format!("{name}: {e}"));
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for case in cases {
        let Some(subject) = subject_of(&case.generators) else {
            continue;
        };
        let result = measure(
            subject,
            path,
            &name,
            case,
            max_size,
            started.elapsed().as_secs_f64(),
            sink,
        );
        out.push(result);
    }
    out
}

fn measure(
    subject: &dyn Subject,
    path: &Path,
    name: &str,
    case: GeneratorCase,
    max_size: usize,
    seconds: f64,
    sink: &mut dyn CaseSink,
) -> CaseResult {
    let (w0, h0) = (case.theirs.w, case.theirs.h);
    let longest = w0.max(h0).max(1);
    let scale = |v: usize| {
        if longest > max_size {
            (v * max_size / longest).max(1)
        } else {
            v
        }
    };
    let (w, h) = (scale(w0), scale(h0));
    let ours = resized(case.ours, w, h);
    let theirs = resized(case.theirs, w, h);
    let mask: Vec<bool> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w * w0 / w, i / w * h0 / h);
            case.mask.get(y * w0 + x).copied().unwrap_or(false)
        })
        .collect();
    let components = if case.color { RGB } else { GRAY };
    let (metrics, errors) = Metrics::measure(&ours, &theirs, components, &mask);
    let measured = Measured {
        metrics,
        size: (w, h),
        reference: format!("texture cache {} / {}", case.set, case.channel),
        reference_size: (w0, h0),
        high: "–".into(),
        notes: vec!["final look of the channel: our imported layers with replacements vs Painter's cache".into()],
        unmapped: Vec::new(),
        settings_hash: String::new(),
        stale: None,
        seconds,
    };
    let result = CaseResult {
        subject: subject.id(),
        project: path.to_path_buf(),
        project_name: name.to_string(),
        set: format!("{} / {}", case.set, case.channel),
        outcome: Outcome::Measured(Box::new(measured)),
    };
    sink.case(
        &result,
        Some(&CaseImages {
            ours: &ours,
            theirs: &theirs,
            mask: &mask,
            errors: &errors,
        }),
    );
    result
}
