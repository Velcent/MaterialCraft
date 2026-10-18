//! [`RunReport`]: what one run of a scenario measured, as named numbers in the order they were taken.
//!
//! Names say their unit: `…_ms` milliseconds, `…_mb` megabytes (10^6 bytes), anything else a count. A spread of
//! measurements is three numbers, `name.mean`, `name.p95` and `name.max` ([`Stat`]). Lower is better for every
//! time and size, which is what [`crate::compare`] relies on.

use materialcraft::json::Value;
use std::time::Duration;

/// How the run ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Measured,
    /// Not run: a source file is missing, or the scenario can't be played on this document.
    Skipped(String),
    Failed(String),
}

/// Mean, 95th percentile and maximum of a list of measurements.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stat {
    pub mean: f64,
    pub p95: f64,
    pub max: f64,
    pub n: usize,
}

impl Stat {
    /// `None` for an empty list. The 95th percentile is the nearest-rank one.
    pub fn of(values: &[f64]) -> Option<Stat> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let n = sorted.len();
        let rank = ((n as f64 * 0.95).ceil() as usize).clamp(1, n) - 1;
        let at = |i: usize| sorted.get(i).copied().unwrap_or(0.0);
        Some(Stat { mean: sorted.iter().sum::<f64>() / n as f64, p95: at(rank), max: at(n - 1), n })
    }

    /// Of durations, in milliseconds.
    pub fn of_durations(values: &[Duration]) -> Option<Stat> {
        Stat::of(&values.iter().map(|d| ms(*d)).collect::<Vec<_>>())
    }
}

/// Milliseconds.
pub fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Megabytes (10^6).
pub fn mb(bytes: usize) -> f64 {
    bytes as f64 / 1e6
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    pub scenario: String,
    pub status: Status,
    /// `release` or `debug`: the runner's build.
    pub profile: String,
    pub metrics: Vec<(String, f64)>,
    /// What the run picked or noticed (the set and layer it worked on, edits it couldn't make).
    pub notes: Vec<String>,
}

impl RunReport {
    pub fn new(scenario: &str) -> RunReport {
        RunReport { scenario: scenario.to_string(), status: Status::Measured, profile: build_profile().into(), metrics: Vec::new(), notes: Vec::new() }
    }

    pub fn skipped(scenario: &str, why: impl Into<String>) -> RunReport {
        RunReport { status: Status::Skipped(why.into()), ..RunReport::new(scenario) }
    }

    pub fn failed(scenario: &str, why: impl Into<String>) -> RunReport {
        RunReport { status: Status::Failed(why.into()), ..RunReport::new(scenario) }
    }

    /// Records `value` as `name` (replacing an earlier one).
    pub fn put(&mut self, name: impl Into<String>, value: f64) {
        let name = name.into();
        let value = if value.is_finite() { value } else { 0.0 };
        match self.metrics.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => self.metrics.push((name, value)),
        }
    }

    pub fn put_ms(&mut self, name: &str, d: Duration) {
        self.put(name, ms(d));
    }

    /// Records `name.mean`, `name.p95` and `name.max`; nothing for an empty list.
    pub fn put_stat(&mut self, name: &str, s: Option<Stat>) {
        if let Some(s) = s {
            self.put(format!("{name}.mean"), s.mean);
            self.put(format!("{name}.p95"), s.p95);
            self.put(format!("{name}.max"), s.max);
        }
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.notes.push(text.into());
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        self.metrics.iter().find(|(n, _)| n == name).map(|(_, v)| *v)
    }

    pub fn to_json(&self) -> Value {
        let v = Value::object().with("scenario", self.scenario.as_str());
        let v = match &self.status {
            Status::Measured => v.with("status", "measured"),
            Status::Skipped(why) => v.with("status", "skipped").with("reason", why.as_str()),
            Status::Failed(why) => v.with("status", "failed").with("reason", why.as_str()),
        };
        let metrics = Value::Object(self.metrics.iter().map(|(k, x)| (k.clone(), Value::from(round(*x)))).collect());
        v.with("profile", self.profile.as_str()).with("metrics", metrics).with("notes", self.notes.iter().map(String::as_str).collect::<Vec<_>>())
    }

    pub fn from_json(v: &Value) -> Result<RunReport, String> {
        let scenario = v["scenario"].as_str().ok_or("a report names its scenario")?.to_string();
        let reason = || v["reason"].as_str().unwrap_or_default().to_string();
        let status = match v["status"].as_str() {
            Some("measured") => Status::Measured,
            Some("skipped") => Status::Skipped(reason()),
            Some("failed") => Status::Failed(reason()),
            other => return Err(format!("unknown status {other:?}")),
        };
        let metrics = v["metrics"].as_object().map(|m| m.iter().filter_map(|(k, x)| Some((k.clone(), x.as_f64()?))).collect()).unwrap_or_default();
        let notes = v["notes"].as_array().map(|n| n.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
        Ok(RunReport { scenario, status, profile: v["profile"].as_str().unwrap_or("unknown").to_string(), metrics, notes })
    }

    /// The report as aligned text lines.
    pub fn table(&self) -> String {
        let mut out = format!("{} [{}]", self.scenario, self.profile);
        match &self.status {
            Status::Measured => {}
            Status::Skipped(why) => out.push_str(&format!(": skipped: {why}")),
            Status::Failed(why) => out.push_str(&format!(": FAILED: {why}")),
        }
        out.push('\n');
        for n in &self.notes {
            out.push_str(&format!("  note: {n}\n"));
        }
        for (k, v) in &self.metrics {
            out.push_str(&format!("  {k:<40} {}\n", format_value(k, *v)));
        }
        out
    }
}

/// A metric's value with its unit.
pub fn format_value(name: &str, v: f64) -> String {
    let unit = unit_of(name);
    match unit {
        "ms" if v >= 100.0 => format!("{v:>10.0} ms"),
        "ms" => format!("{v:>10.2} ms"),
        "MB" => format!("{v:>10.2} MB"),
        _ => format!("{v:>10}"),
    }
}

/// `ms`, `MB` or `` (a count), from the name.
pub fn unit_of(name: &str) -> &'static str {
    let base = name.rsplit_once('.').filter(|(_, last)| matches!(*last, "mean" | "p95" | "max")).map_or(name, |(b, _)| b);
    if base.ends_with("_ms") {
        "ms"
    } else if base.ends_with("_mb") {
        "MB"
    } else {
        ""
    }
}

/// `release` unless debug assertions are on.
pub fn build_profile() -> &'static str {
    if cfg!(debug_assertions) { "debug" } else { "release" }
}

/// Four decimals are plenty for milliseconds and megabytes.
fn round(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_of_a_list() {
        let s = Stat::of(&(1..=100).map(f64::from).collect::<Vec<_>>()).unwrap();
        assert_eq!((s.mean, s.p95, s.max, s.n), (50.5, 95.0, 100.0, 100));
        assert_eq!(Stat::of(&[3.0]).unwrap().p95, 3.0);
        assert!(Stat::of(&[]).is_none());
    }

    #[test]
    fn report_round_trips() {
        let mut r = RunReport::new("x");
        r.put("open_ms", 12.5);
        r.put_stat("stamp_ms", Stat::of(&[1.0, 2.0]));
        r.note("set Body");
        let back = RunReport::from_json(&r.to_json()).unwrap();
        assert_eq!(back, r);
        let s = RunReport::skipped("y", "missing");
        assert_eq!(RunReport::from_json(&s.to_json()).unwrap().status, Status::Skipped("missing".into()));
        assert_eq!(unit_of("stamp_ms.p95"), "ms");
        assert_eq!(unit_of("upload_mb"), "MB");
        assert_eq!(unit_of("dabs"), "");
    }
}
