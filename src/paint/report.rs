//! [`ImportReport`]: what an importer translated exactly, approximated, baked or kept untouched.

/// How faithfully one item was carried over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fidelity {
    /// Same result as in the source application.
    Exact,
    /// Translated to the nearest equivalent; the result may differ.
    Approximated,
    /// Kept only as pixels (e.g. a procedural effect flattened to an image); no longer editable.
    Baked,
    /// Not translated; the original bytes are kept in [`crate::paint::ForeignFile`]s.
    KeptRaw,
}

impl Fidelity {
    pub fn id(self) -> &'static str {
        match self {
            Fidelity::Exact => "exact",
            Fidelity::Approximated => "approximated",
            Fidelity::Baked => "baked",
            Fidelity::KeptRaw => "kept_raw",
        }
    }

    pub fn from_id(id: &str) -> Option<Fidelity> {
        [Fidelity::Exact, Fidelity::Approximated, Fidelity::Baked, Fidelity::KeptRaw].into_iter().find(|f| f.id() == id)
    }
}

/// One line of the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportEntry {
    /// What the entry is about, e.g. `texture set "Body" / layer "Rust"`.
    pub item: String,
    pub fidelity: Fidelity,
    /// Human-readable detail; may be empty.
    pub note: String,
}

/// The result report of an import.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportReport {
    /// What was imported, e.g. `substance-painter 9.0.0: robot.spp`.
    pub source: String,
    pub entries: Vec<ReportEntry>,
}

impl ImportReport {
    pub fn new(source: impl Into<String>) -> ImportReport {
        ImportReport { source: source.into(), entries: Vec::new() }
    }

    pub fn add(&mut self, item: impl Into<String>, fidelity: Fidelity, note: impl Into<String>) {
        self.entries.push(ReportEntry { item: item.into(), fidelity, note: note.into() });
    }

    /// Number of entries with `fidelity`.
    pub fn count(&self, fidelity: Fidelity) -> usize {
        self.entries.iter().filter(|e| e.fidelity == fidelity).count()
    }

    /// True if everything was translated exactly.
    pub fn is_exact(&self) -> bool {
        self.entries.iter().all(|e| e.fidelity == Fidelity::Exact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        let mut r = ImportReport::new("test");
        assert!(r.is_exact());
        r.add("layer a", Fidelity::Exact, "");
        r.add("layer b", Fidelity::Baked, "procedural mask");
        assert_eq!(r.count(Fidelity::Baked), 1);
        assert!(!r.is_exact());
        assert_eq!(Fidelity::from_id(Fidelity::KeptRaw.id()), Some(Fidelity::KeptRaw));
    }
}
