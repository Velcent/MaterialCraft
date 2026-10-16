//! [`ReportText`]: an import report as plain text, one line per entry, grouped totals at the end.

use materialcraft::paint::{Fidelity, ImportReport};
use std::fmt;

pub struct ReportText<'a>(pub &'a ImportReport);

impl fmt::Display for ReportText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.0.source)?;
        for e in &self.0.entries {
            writeln!(f, "  {:<12} {}: {}", e.fidelity.id(), e.item, e.note)?;
        }
        let all = [
            Fidelity::Exact,
            Fidelity::Approximated,
            Fidelity::Baked,
            Fidelity::KeptRaw,
        ];
        let totals: Vec<String> = all.iter().map(|x| format!("{} {}", self.0.count(*x), x.id())).collect();
        writeln!(f, "  total: {}", totals.join(", "))
    }
}
