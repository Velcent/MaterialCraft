//! `import-report.json`: `{"source", "entries": [{"item", "fidelity", "note"}]}`.

use super::fields::Obj;
use crate::json::Value;
use crate::paint::format::FormatError;
use crate::paint::{Fidelity, ImportReport, ReportEntry};

pub fn write(report: &ImportReport) -> Value {
    let entries: Value =
        report.entries.iter().map(|e| Value::object().with("item", e.item.as_str()).with("fidelity", e.fidelity.id()).with("note", e.note.as_str())).collect();
    Value::object().with("source", report.source.as_str()).with("entries", entries)
}

pub fn read(v: &Value) -> Result<ImportReport, FormatError> {
    let mut o = Obj::new(v, "import-report.json")?;
    let mut report = ImportReport::new(o.str("source")?);
    for (i, e) in o.array("entries")?.iter().enumerate() {
        let mut eo = Obj::new(e, format!("import-report.json entries[{i}]"))?;
        let f = eo.str("fidelity")?;
        let fidelity = Fidelity::from_id(f).ok_or_else(|| eo.error("fidelity", format!("unknown fidelity {f:?}")))?;
        report.entries.push(ReportEntry { item: eo.str("item")?.to_string(), fidelity, note: eo.opt_str("note")?.unwrap_or_default().to_string() });
    }
    Ok(report)
}
