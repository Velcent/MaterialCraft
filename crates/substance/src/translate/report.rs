//! [`Reporter`]: the wording of the import report, one method per kind of element.

use super::foreign::KeptFiles;
use materialcraft::paint::{Fidelity, ImportReport};

const MB: f64 = 1024.0 * 1024.0;

pub struct Reporter {
    report: ImportReport,
}

impl Reporter {
    pub fn new(source: String) -> Self {
        Reporter {
            report: ImportReport::new(source),
        }
    }

    pub fn finish(self) -> ImportReport {
        self.report
    }

    pub fn add(&mut self, item: impl Into<String>, fidelity: Fidelity, note: impl Into<String>) {
        self.report.add(item, fidelity, note);
    }

    pub fn mesh(&mut self, parts: usize, triangles: usize) {
        self.add(
            "mesh",
            Fidelity::Approximated,
            format!(
                "{parts} parts, {triangles} triangles; at Painter's normalised scale (original placement kept raw)"
            ),
        );
    }

    pub fn set(&mut self, set: &str, note: String) {
        self.add(format!("texture set \"{set}\""), Fidelity::Exact, note);
    }

    pub fn set_problem(&mut self, set: &str, fidelity: Fidelity, note: impl Into<String>) {
        self.add(format!("texture set \"{set}\""), fidelity, note);
    }

    /// A flattened channel: `Baked` (pixels only), or `Approximated` when the stored block was short.
    pub fn channel(&mut self, set: &str, channel: &str, missing_bytes: usize) {
        let item = format!("texture set \"{set}\" / channel {channel}");
        match missing_bytes {
            0 => self.add(item, Fidelity::Baked, "flattened layer stack from the texture cache"),
            n => self.add(
                item,
                Fidelity::Approximated,
                format!("stored data {n} bytes short, padded with zeros"),
            ),
        }
    }

    pub fn channel_failed(&mut self, set: &str, channel: &str, reason: impl Into<String>) {
        self.add(
            format!("texture set \"{set}\" / channel {channel}"),
            Fidelity::KeptRaw,
            reason,
        );
    }

    pub fn baked_map(&mut self, set: &str, name: &str, missing_bytes: usize) {
        let item = format!("texture set \"{set}\" / baked map \"{name}\"");
        match missing_bytes {
            0 => self.add(item, Fidelity::Exact, ""),
            n => self.add(
                item,
                Fidelity::Approximated,
                format!("stored data {n} bytes short, padded with zeros"),
            ),
        }
    }

    pub fn baked_failed(&mut self, set: &str, name: &str, reason: impl Into<String>) {
        self.add(
            format!("texture set \"{set}\" / baked map \"{name}\""),
            Fidelity::KeptRaw,
            reason,
        );
    }

    pub fn layers(&mut self, set: &str, layers: usize, actions: usize) {
        self.add(
            format!("texture set \"{set}\" / layer stack"),
            Fidelity::KeptRaw,
            format!("layer stack flattened: {layers} layers with {actions} effects in document.bin not yet translated"),
        );
    }

    pub fn unused_material(&mut self, name: &str) {
        self.add(
            format!("texture set \"{name}\""),
            Fidelity::KeptRaw,
            "in document.bin but no mesh slot uses it (left over from a replaced mesh); not imported",
        );
    }

    pub fn foreign(&mut self, kept: &KeptFiles) {
        self.add(
            "foreign files",
            Fidelity::KeptRaw,
            format!(
                "{} files ({:.1} MB) kept verbatim under foreign/substance for later re-conversion",
                kept.files.len(),
                kept.bytes as f64 / MB
            ),
        );
        for (path, reason) in &kept.failed {
            self.add(
                format!("foreign file {path}"),
                Fidelity::Approximated,
                format!("not kept: {reason}"),
            );
        }
    }
}
