//! [`History`]: one JSON line per scenario run, appended to `perf/history.jsonl` in the internal docs (outside the
//! repository), like mcparity's history. Each line is a [`RunReport`]'s JSON plus when, where and from which code:
//!
//! ```json
//! {"scenario": "stroke-2k", "status": "measured", "profile": "release", "metrics": {"open_ms": 812.3, ..},
//!  "notes": [..], "time": "2026-10-18T20:15:02Z", "commit": "ed9877e", "dirty": false, "repeat": 1,
//!  "machine": {"host": "..", "cpu": "AMD Ryzen 7 9800X3D 8-Core Processor", "cores": 16}}
//! ```

use crate::machine::Machine;
use crate::report::RunReport;
use materialcraft::json::{self, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const HISTORY_ENV: &str = "MC_PERF_HISTORY";

/// One line of the history.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// ISO 8601, UTC.
    pub time: String,
    /// Short hash (`unknown` outside a checkout).
    pub commit: String,
    /// Tracked files had changes.
    pub dirty: bool,
    pub machine: Machine,
    /// The repetitions the scenario was played with.
    pub repeat: u32,
    pub report: RunReport,
}

impl Record {
    pub fn to_json(&self) -> Value {
        self.report
            .to_json()
            .with("time", self.time.as_str())
            .with("commit", self.commit.as_str())
            .with("dirty", self.dirty)
            .with("repeat", self.repeat)
            .with("machine", self.machine.to_json())
    }

    pub fn from_json(v: &Value) -> Result<Record, String> {
        Ok(Record {
            time: v["time"].as_str().unwrap_or_default().to_string(),
            commit: v["commit"].as_str().unwrap_or("unknown").to_string(),
            dirty: v["dirty"].as_bool().unwrap_or(false),
            machine: Machine::from_json(&v["machine"]),
            repeat: v["repeat"].as_u64().and_then(|r| u32::try_from(r).ok()).unwrap_or(1),
            report: RunReport::from_json(v)?,
        })
    }
}

pub struct History {
    pub path: PathBuf,
}

impl History {
    /// `$MC_PERF_HISTORY`, else `<docs>/perf/history.jsonl`; `None` when neither is available.
    pub fn default_location() -> Option<History> {
        let path = match std::env::var_os(HISTORY_ENV) {
            Some(p) => PathBuf::from(p),
            None => materialcraft_parity::provenance::docs_dir()?.join("perf").join("history.jsonl"),
        };
        Some(History { path })
    }

    pub fn at(path: &Path) -> History {
        History { path: path.to_path_buf() }
    }

    pub fn append(&self, record: &Record) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        file.write_all(format!("{}\n", record.to_json()).as_bytes())
    }

    /// Every readable line, oldest first (a missing file is an empty history; unreadable lines are skipped).
    pub fn read(&self) -> std::io::Result<Vec<Record>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        Ok(text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| json::parse(l).ok()).filter_map(|v| Record::from_json(&v).ok()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_and_reads_lines() {
        let path = std::env::temp_dir().join(format!("mcperf-history-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut report = RunReport::new("stroke-2k");
        report.put("open_ms", 10.0);
        let r = Record { time: "t".into(), commit: "abc".into(), dirty: true, machine: Machine::this(), repeat: 2, report };
        let h = History::at(&path);
        h.append(&r).unwrap();
        h.append(&r).unwrap();
        let back = h.read().unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0], r);
        std::fs::remove_file(&path).unwrap();
        assert!(h.read().unwrap().is_empty());
    }
}
