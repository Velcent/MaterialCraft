//! [`ReplacementAccuracy`]: how close MaterialCraft's replacement generators came to Painter, as measured by
//! `mcparity run generator…` on the user's own projects. The numbers are read at run time from mcparity's
//! history (`$MC_PARITY_HISTORY`, else `parity/history.jsonl` in the internal docs next to the checkout);
//! nothing measured is built into the program.
//!
//! Per generator the latest run that measured it counts: the median of its cases' MAE (8-bit steps).

use materialcraft::json::{self, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The environment variable mcparity also reads.
pub const HISTORY_ENV: &str = "MC_PARITY_HISTORY";
/// History lines of a generator's subject are `generator.<generator id>`.
pub const SUBJECT_PREFIX: &str = "generator.";

/// One generator's measured accuracy.
#[derive(Debug, Clone, PartialEq)]
pub struct Accuracy {
    /// Median of the cases' mean absolute error, in 8-bit steps (1 = 1/255).
    pub median_mae: f64,
    /// Measured cases in that run.
    pub cases: usize,
    /// The mcparity run id.
    pub run: String,
}

/// Accuracy per generator id.
#[derive(Debug, Clone, Default)]
pub struct ReplacementAccuracy {
    by_generator: BTreeMap<String, Accuracy>,
}

impl ReplacementAccuracy {
    /// From the default history location; empty when there is none.
    pub fn load() -> ReplacementAccuracy {
        match history_path() {
            Some(p) => ReplacementAccuracy::read(&p),
            None => ReplacementAccuracy::default(),
        }
    }

    /// From a history file; empty when it cannot be read.
    pub fn read(path: &Path) -> ReplacementAccuracy {
        std::fs::read_to_string(path)
            .map(|text| ReplacementAccuracy::parse(&text))
            .unwrap_or_default()
    }

    /// From history lines (JSON, one per case).
    pub fn parse(text: &str) -> ReplacementAccuracy {
        // generator → (latest run, its MAEs); runs are named by time, so the largest is the latest
        let mut runs: BTreeMap<String, (String, Vec<f64>)> = BTreeMap::new();
        for line in text.lines() {
            let Ok(v) = json::parse(line) else { continue };
            let Some(id) = v["subject"].as_str().and_then(|s| s.strip_prefix(SUBJECT_PREFIX)) else {
                continue;
            };
            let (Some(run), Some("measured"), Some(mae)) = (v["run"].as_str(), v["status"].as_str(), v["mae"].as_f64())
            else {
                continue;
            };
            let entry = runs
                .entry(id.to_string())
                .or_insert_with(|| (run.to_string(), Vec::new()));
            if run > entry.0.as_str() {
                *entry = (run.to_string(), Vec::new());
            }
            if run == entry.0 {
                entry.1.push(mae);
            }
        }
        let by_generator = runs
            .into_iter()
            .filter_map(|(id, (run, maes))| {
                let median = median(maes.clone())?;
                Some((
                    id,
                    Accuracy {
                        median_mae: median,
                        cases: maes.len(),
                        run,
                    },
                ))
            })
            .collect();
        ReplacementAccuracy { by_generator }
    }

    /// The accuracy of generator `id`, if measured.
    pub fn of(&self, id: &str) -> Option<Accuracy> {
        self.by_generator.get(id).cloned()
    }
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    values.retain(|v| v.is_finite());
    values.sort_by(f64::total_cmp);
    let n = values.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => values.get(n / 2).copied(),
        _ => Some((values.get(n / 2 - 1)? + values.get(n / 2)?) / 2.0),
    }
}

/// `$MC_PARITY_HISTORY`, else `MaterialCraft-docs/parity/history.jsonl` next to the main checkout of the
/// git repository around the current directory.
fn history_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(HISTORY_ENV) {
        return Some(PathBuf::from(p));
    }
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()?;
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let docs = common.parent()?.parent()?.join("MaterialCraft-docs");
    let path = docs.join("parity").join("history.jsonl");
    path.is_file().then_some(path)
}

/// Writes one accuracy as `5.3/255 (12 cases)`.
pub fn describe(a: &Accuracy) -> String {
    format!("{:.1}/255 median over {} measured cases", a.median_mae, a.cases)
}

impl Accuracy {
    pub fn to_json(&self) -> Value {
        Value::object()
            .with("median_mae", self.median_mae)
            .with("cases", self.cases)
            .with("run", self.run.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_run_median_per_generator() {
        let text = [
            r#"{"run":"20261001-1","subject":"generator.dirt","status":"measured","mae":9.0}"#,
            r#"{"run":"20261002-1","subject":"generator.dirt","status":"measured","mae":3.0}"#,
            r#"{"run":"20261002-1","subject":"generator.dirt","status":"measured","mae":5.0}"#,
            r#"{"run":"20261002-1","subject":"generator.dirt","status":"skipped"}"#,
            r#"{"run":"20261002-1","subject":"generator.dirt","status":"measured","mae":4.0}"#,
            r#"{"run":"20261003-1","subject":"baked.ao","status":"measured","mae":1.0}"#,
            "not json",
        ]
        .join("\n");
        let a = ReplacementAccuracy::parse(&text);
        let dirt = a.of("dirt").unwrap();
        assert_eq!((dirt.median_mae, dirt.cases, dirt.run.as_str()), (4.0, 3, "20261002-1"));
        assert!(a.of("ao").is_none() && a.of("baked.ao").is_none());
        assert_eq!(median(vec![1.0, 3.0]), Some(2.0));
        assert_eq!(median(vec![]), None);
    }
}
