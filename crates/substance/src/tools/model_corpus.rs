//! `mcsub model --all <folder>`: builds the model of every `.spp` below a folder and prints one line per
//! project (layers, recognised/raw/unmirrored objects) plus totals over the corpus.

use crate::inspect::Corpus;
use crate::model::{ContentKind, ModelReport, StackSummary};
use crate::project::SppProject;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

/// Totals over all projects that built a model.
#[derive(Default)]
struct Totals {
    projects: usize,
    failures: usize,
    sets: usize,
    objects: usize,
    recognised: usize,
    raw: usize,
    not_modelled: usize,
    summary: StackSummary,
    raw_classes: BTreeMap<String, usize>,
}

impl Totals {
    fn add(&mut self, sets: usize, report: &ModelReport, summary: &StackSummary) {
        self.projects += 1;
        self.sets += sets;
        self.objects += report.total();
        self.recognised += report.recognised();
        self.raw += report.raw();
        self.not_modelled += report.not_modelled();
        for (c, n) in report.raw_classes() {
            *self.raw_classes.entry(c.to_string()).or_default() += n;
        }
        let s = &mut self.summary;
        s.layers += summary.layers;
        s.folders += summary.folders;
        s.fill_layers += summary.fill_layers;
        s.paint_layers += summary.paint_layers;
        s.other_layers += summary.other_layers;
        s.hidden += summary.hidden;
        merge(&mut s.content, &summary.content);
        merge(&mut s.masks, &summary.masks);
    }
}

fn merge(into: &mut BTreeMap<ContentKind, usize>, from: &BTreeMap<ContentKind, usize>) {
    for (k, n) in from {
        *into.entry(*k).or_default() += n;
    }
}

/// Runs over every `.spp` below `root`; `progress` gets each path before it is read.
pub fn model_all(root: &Path, progress: &mut dyn FnMut(&Path)) -> String {
    let corpus = Corpus::discover(root);
    let mut out = String::new();
    let mut totals = Totals::default();
    let _ = writeln!(
        out,
        "{:<64} {:>4} {:>4} {:>6} {:>9} {:>9} {:>5}",
        "project", "fmt", "sets", "layers", "objects", "recogn.", "raw"
    );
    for path in corpus.files() {
        progress(path);
        let name = path.strip_prefix(root).unwrap_or(path).display().to_string();
        match SppProject::open(path).and_then(|p| p.model()) {
            Ok((doc, report)) => {
                let summary = StackSummary::of(&doc);
                let _ = writeln!(
                    out,
                    "{name:<64} {:>4} {:>4} {:>6} {:>9} {:>9} {:>5}",
                    doc.format_version,
                    doc.texture_sets.len(),
                    summary.layers,
                    report.total(),
                    report.recognised(),
                    report.raw()
                );
                totals.add(doc.texture_sets.len(), &report, &summary);
            }
            Err(e) => {
                totals.failures += 1;
                let _ = writeln!(out, "{name:<64} FAILED {e}");
            }
        }
    }
    write_totals(&mut out, &totals);
    out
}

fn write_totals(out: &mut String, t: &Totals) {
    let s = &t.summary;
    let kinds = |m: &BTreeMap<ContentKind, usize>| {
        m.iter()
            .map(|(k, n)| format!("{} {n}", k.name()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let _ = writeln!(
        out,
        "\n{} projects built, {} failed · {} texture sets · {} layers ({} folders, {} fill, {} paint, {} other, {} hidden)",
        t.projects, t.failures, t.sets, s.layers, s.folders, s.fill_layers, s.paint_layers, s.other_layers, s.hidden
    );
    let _ = writeln!(
        out,
        "objects {} · recognised {} · raw {} · not mirrored {}",
        t.objects, t.recognised, t.raw, t.not_modelled
    );
    let _ = writeln!(out, "layer content: {}", kinds(&s.content));
    let _ = writeln!(out, "masks: {}", kinds(&s.masks));
    if !t.raw_classes.is_empty() {
        let raw: Vec<String> = t.raw_classes.iter().map(|(c, n)| format!("{c} {n}")).collect();
        let _ = writeln!(out, "raw classes: {}", raw.join(", "));
    }
}
