//! [`History`]: one JSON line per case and run, appended to `parity/history.jsonl` in the internal docs, so
//! progress over time is visible. Numbers only: no images, no pixels, nothing from the projects but names.

use crate::provenance;
use crate::run::{CaseResult, Outcome};
use materialcraft::json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const HISTORY_ENV: &str = "MC_PARITY_HISTORY";

pub struct History {
    pub path: PathBuf,
}

/// What every line of one run shares.
pub struct RunStamp<'a> {
    pub run: &'a str,
    pub time: &'a str,
    pub commit: &'a str,
    pub max_size: usize,
}

impl History {
    /// `$MC_PARITY_HISTORY`, else `<docs>/parity/history.jsonl`; `None` when neither is available.
    pub fn default_location() -> Option<History> {
        let path = match std::env::var_os(HISTORY_ENV) {
            Some(p) => PathBuf::from(p),
            None => provenance::docs_dir()?.join("parity").join("history.jsonl"),
        };
        Some(History { path })
    }

    pub fn at(path: &Path) -> History {
        History {
            path: path.to_path_buf(),
        }
    }

    /// Appends one line per result.
    pub fn append(&self, stamp: &RunStamp, results: &[CaseResult]) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.path)?;
        let mut text = String::new();
        for r in results {
            text.push_str(&line(stamp, r).to_string());
            text.push('\n');
        }
        file.write_all(text.as_bytes())
    }
}

/// One case as JSON.
pub fn line(stamp: &RunStamp, r: &CaseResult) -> Value {
    let v = Value::object()
        .with("time", stamp.time)
        .with("run", stamp.run)
        .with("commit", stamp.commit)
        .with("subject", r.subject)
        .with("project", r.project.display().to_string())
        .with("set", r.set.as_str())
        .with("max_size", stamp.max_size);
    match &r.outcome {
        Outcome::Skipped(why) => v.with("status", "skipped").with("reason", why.as_str()),
        Outcome::Failed(why) => v.with("status", "failed").with("reason", why.as_str()),
        Outcome::Measured(m) => {
            let mut components = Value::object();
            for c in &m.metrics.components {
                components.insert(
                    c.label,
                    Value::object()
                        .with("mae", round(c.diff.mae * 255.0))
                        .with("max", round(c.diff.max * 255.0))
                        .with("within", round(c.diff.within))
                        .with("ssim", c.ssim.map_or(Value::Null, |s| round(s).into()))
                        .with("gain", round(c.fit.gain))
                        .with("offset", round(c.fit.offset)),
                );
            }
            let d = &m.metrics.overall;
            let status = match &m.stale {
                Some(why) => v.with("status", "skipped").with("reason", why.as_str()),
                None => v.with("status", "measured"),
            };
            status
                .with("size", Value::Array(vec![m.size.0.into(), m.size.1.into()]))
                .with("mae", round(m.metrics.mae_steps()))
                .with("max", round(d.max * 255.0))
                .with(
                    "psnr",
                    if d.psnr.is_finite() {
                        round(d.psnr).into()
                    } else {
                        Value::Null
                    },
                )
                .with("within", round(d.within))
                .with("ssim", m.metrics.ssim.map_or(Value::Null, |s| round(s).into()))
                .with("histogram", round(m.metrics.distribution.histogram))
                .with(
                    "spectrum",
                    m.metrics.distribution.spectrum.map_or(Value::Null, |s| round(s).into()),
                )
                .with("texels", d.texels)
                .with("components", components)
                .with("high", m.high.as_str())
                .with("settings_hash", m.settings_hash.as_str())
                .with("seconds", round(m.seconds))
        }
    }
}

/// Four decimals are plenty for 8-bit steps and percentages.
fn round(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines() {
        let path = std::env::temp_dir().join(format!("mcparity-history-{}.jsonl", std::process::id()));
        let r = CaseResult {
            subject: "baked.ao",
            project: PathBuf::from("/x/a.spp"),
            project_name: "a.spp".into(),
            set: "Cup".into(),
            outcome: Outcome::Skipped("no high-poly".into()),
        };
        let stamp = RunStamp {
            run: "r1",
            time: "t",
            commit: "c",
            max_size: 512,
        };
        let h = History::at(&path);
        h.append(&stamp, std::slice::from_ref(&r)).unwrap();
        h.append(&stamp, &[r]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        let v = materialcraft::json::parse(text.lines().next().unwrap()).unwrap();
        assert_eq!(v["status"].as_str(), Some("skipped"));
        assert_eq!(v["set"].as_str(), Some("Cup"));
        std::fs::remove_file(&path).unwrap();
    }
}
