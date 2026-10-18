//! Comparing a run with the previous run of the same scenario on the same machine and build profile: every time
//! and size, with regressions over a threshold flagged. Maximums are shown but never flagged (one slow frame is
//! noise); differences under a floor ([`FLOOR_MS`], [`FLOOR_MB`]) aren't flagged either.

use crate::history::Record;
use crate::report::{Status, format_value, unit_of};

/// Differences smaller than this many milliseconds are noise.
pub const FLOOR_MS: f64 = 0.5;
/// Differences smaller than this many megabytes are noise.
pub const FLOOR_MB: f64 = 0.1;
/// The default threshold, a fraction of the earlier value.
pub const THRESHOLD: f64 = 0.10;

#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Same,
    Better,
    Regression,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub metric: String,
    pub before: f64,
    pub after: f64,
    pub verdict: Verdict,
}

impl Change {
    /// The change as a fraction of the earlier value.
    pub fn relative(&self) -> f64 {
        if self.before.abs() < 1e-9 { 0.0 } else { (self.after - self.before) / self.before }
    }
}

#[derive(Debug, Clone)]
pub struct Comparison<'a> {
    pub before: &'a Record,
    pub after: &'a Record,
    pub changes: Vec<Change>,
}

impl Comparison<'_> {
    pub fn regressions(&self) -> usize {
        self.changes.iter().filter(|c| c.verdict == Verdict::Regression).count()
    }

    pub fn text(&self) -> String {
        let stamp = |r: &Record| format!("{}{} {}", r.commit, if r.dirty { "+dirty" } else { "" }, r.time);
        let mut out = format!("{}: {} -> {}\n", self.after.report.scenario, stamp(self.before), stamp(self.after));
        for c in &self.changes {
            let mark = match c.verdict {
                Verdict::Same => "",
                Verdict::Better => "  better",
                Verdict::Regression => "  REGRESSION",
            };
            out.push_str(&format!(
                "  {:<40} {} -> {} {:>+7.1}%{mark}\n",
                c.metric,
                format_value(&c.metric, c.before),
                format_value(&c.metric, c.after),
                c.relative() * 100.0
            ));
        }
        out
    }
}

/// The last two measured runs of `scenario` on `after`'s machine and profile, ending with the newest (`None` with
/// fewer than two).
pub fn last_two<'a>(records: &'a [Record], scenario: &str) -> Option<(&'a Record, &'a Record)> {
    let measured: Vec<&Record> = records.iter().filter(|r| r.report.scenario == scenario && r.report.status == Status::Measured).collect();
    let after = *measured.last()?;
    let before = measured.iter().rev().skip(1).find(|r| r.machine.same_as(&after.machine) && r.report.profile == after.report.profile)?;
    Some((before, after))
}

/// Every time and size both runs have, in `after`'s order.
pub fn compare<'a>(before: &'a Record, after: &'a Record, threshold: f64) -> Comparison<'a> {
    let changes = after
        .report
        .metrics
        .iter()
        .filter_map(|(name, a)| {
            let floor = match unit_of(name) {
                "ms" => FLOOR_MS,
                "MB" => FLOOR_MB,
                _ => return None,
            };
            let b = before.report.get(name)?;
            let flagged = !name.ends_with(".max") && (a - b).abs() >= floor;
            let verdict = match () {
                _ if flagged && *a > b * (1.0 + threshold) => Verdict::Regression,
                _ if flagged && *a < b * (1.0 - threshold) => Verdict::Better,
                _ => Verdict::Same,
            };
            Some(Change { metric: name.clone(), before: b, after: *a, verdict })
        })
        .collect();
    Comparison { before, after, changes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;
    use crate::report::RunReport;

    fn record(commit: &str, host: &str, values: &[(&str, f64)]) -> Record {
        let mut report = RunReport::new("s");
        for (k, v) in values {
            report.put(*k, *v);
        }
        Record { time: "t".into(), commit: commit.into(), dirty: false, machine: Machine { host: host.into(), cpu: "c".into(), cores: 1 }, repeat: 1, report }
    }

    #[test]
    fn flags_regressions_over_the_threshold() {
        let a = record("a", "h", &[("open_ms", 100.0), ("x_ms.mean", 1.0), ("x_ms.max", 1.0), ("dabs", 10.0), ("up_mb", 5.0)]);
        let b = record("b", "h", &[("open_ms", 130.0), ("x_ms.mean", 1.3), ("x_ms.max", 9.0), ("dabs", 20.0), ("up_mb", 2.0)]);
        let c = compare(&a, &b, THRESHOLD);
        let verdict = |m: &str| c.changes.iter().find(|x| x.metric == m).map(|x| x.verdict.clone());
        assert_eq!(verdict("open_ms"), Some(Verdict::Regression));
        assert_eq!(verdict("x_ms.mean"), Some(Verdict::Same), "under the floor");
        assert_eq!(verdict("x_ms.max"), Some(Verdict::Same), "maximums aren't flagged");
        assert_eq!(verdict("dabs"), None, "counts aren't compared");
        assert_eq!(verdict("up_mb"), Some(Verdict::Better));
        assert_eq!(c.regressions(), 1);
        assert!(c.text().contains("REGRESSION"));
    }

    #[test]
    fn pairs_runs_on_the_same_machine() {
        let records = vec![record("1", "h", &[]), record("2", "other", &[]), record("3", "h", &[])];
        let (before, after) = last_two(&records, "s").unwrap();
        assert_eq!((before.commit.as_str(), after.commit.as_str()), ("1", "3"));
        assert!(last_two(&records[..2], "s").is_none());
    }
}
