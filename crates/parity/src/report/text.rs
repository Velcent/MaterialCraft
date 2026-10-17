//! Plain-text tables for the terminal: one row per measured case, the skip reasons, and a summary per subject.

use super::summary::{fmt_opt, SubjectSummary};
use crate::run::survey::generalise;
use crate::run::{CaseResult, Outcome};
use std::fmt::Write;

/// Measured cases, then skip/failure reasons (counted), then the per-subject summary.
pub fn table(results: &[CaseResult]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:<20} {:<44} {:>9} {:>7} {:>6} {:>6} {:>7} {:>6} {:>13}",
        "subject", "project / set", "size", "MAE", "max", "PSNR", "≤2/255", "SSIM", "fit gain/off"
    );
    for r in results {
        let Some(m) = r.measured() else { continue };
        let d = &m.metrics.overall;
        let fit = m.metrics.components.first().map(|c| c.fit);
        let _ = writeln!(
            out,
            "{:<20} {:<44} {:>9} {:>7.2} {:>6.0} {:>6.1} {:>6.1}% {:>6} {:>13}",
            r.subject,
            clip(&r.label(), 44),
            format!("{}x{}", m.size.0, m.size.1),
            m.metrics.mae_steps(),
            d.max * 255.0,
            d.psnr.min(99.9),
            d.within,
            fmt_opt(m.metrics.ssim, 3),
            fit.map_or("–".into(), |f| format!("{:.2}/{:.2}", f.gain, f.offset)),
        );
    }
    let reasons = reasons(results);
    if !reasons.is_empty() {
        let _ = writeln!(out, "\nnot measured:");
        for ((subject, kind, reason), n) in reasons {
            let _ = writeln!(out, "  {subject:<20} {n:>4} × {kind}: {reason}");
        }
    }
    let _ = writeln!(out, "\nsummary (MAE in 8-bit steps):");
    let _ = writeln!(
        out,
        "  {:<20} {:>8} {:>7} {:>6} {:>9} {:>10} {:>7} {:>6}",
        "subject", "measured", "skipped", "failed", "MAE mean", "MAE median", "≤2/255", "SSIM"
    );
    for s in SubjectSummary::of(results) {
        let _ = writeln!(
            out,
            "  {:<20} {:>8} {:>7} {:>6} {:>9} {:>10} {:>6}% {:>6}",
            s.subject,
            s.measured,
            s.skipped,
            s.failed,
            fmt_opt(s.mae_mean, 2),
            fmt_opt(s.mae_median, 2),
            fmt_opt(s.within_mean, 1),
            fmt_opt(s.ssim_mean, 3),
        );
    }
    out
}

/// (subject, skipped/failed, reason) → count, most frequent first.
fn reasons(results: &[CaseResult]) -> Vec<((&'static str, &'static str, String), usize)> {
    let mut counts: Vec<((&'static str, &'static str, String), usize)> = Vec::new();
    for r in results {
        let key = match &r.outcome {
            Outcome::Skipped(why) => (r.subject, "skipped", generalise(why)),
            Outcome::Failed(why) => (r.subject, "failed", why.clone()),
            Outcome::Measured(_) => continue,
        };
        match counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key, 1)),
        }
    }
    counts.sort_by(|a, b| a.0 .0.cmp(b.0 .0).then(b.1.cmp(&a.1)));
    counts
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n - 1).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn lists_reasons_and_summary_without_measurements() {
        let r = |why: &str| CaseResult {
            subject: "baked.ao",
            project: PathBuf::from("a.spp"),
            project_name: "a.spp".into(),
            set: "s".into(),
            outcome: Outcome::Skipped(why.into()),
        };
        let t = table(&[r("high-poly 'x.obj' not found"), r("high-poly 'y.obj' not found")]);
        assert!(t.contains("2 × skipped: high-poly '…' not found"));
        assert!(t.contains("baked.ao"));
        assert_eq!(clip("abcdef", 4), "abc…");
    }
}
