//! [`SubjectSummary`]: one line per subject over a run: how many cases were measured, skipped or failed, and
//! the mean and median of the main numbers.

use crate::run::{CaseResult, Outcome};

#[derive(Debug, Clone, PartialEq)]
pub struct SubjectSummary {
    pub subject: &'static str,
    pub measured: usize,
    pub skipped: usize,
    pub failed: usize,
    /// Mean absolute error in 8-bit steps: mean and median over the measured cases.
    pub mae_mean: Option<f64>,
    pub mae_median: Option<f64>,
    /// Share of texels within 2/255, mean over cases, in percent.
    pub within_mean: Option<f64>,
    pub ssim_mean: Option<f64>,
    /// Texture statistics (look-alikes): median histogram distance (8-bit steps) and spectrum distance.
    pub histogram_median: Option<f64>,
    pub spectrum_median: Option<f64>,
}

impl SubjectSummary {
    /// One summary per subject, in the order subjects first appear in `results`.
    pub fn of(results: &[CaseResult]) -> Vec<SubjectSummary> {
        let mut order: Vec<&'static str> = Vec::new();
        results.iter().for_each(|r| {
            if !order.contains(&r.subject) {
                order.push(r.subject);
            }
        });
        order.into_iter().map(|s| Self::for_subject(s, results)).collect()
    }

    fn for_subject(subject: &'static str, results: &[CaseResult]) -> SubjectSummary {
        let mine: Vec<&CaseResult> = results.iter().filter(|r| r.subject == subject).collect();
        let count = |f: fn(&Outcome) -> bool| mine.iter().filter(|r| f(&r.outcome)).count();
        let measured: Vec<_> = mine.iter().filter_map(|r| r.measured()).collect();
        let mut maes: Vec<f64> = measured.iter().map(|m| m.metrics.mae_steps()).collect();
        maes.sort_by(f64::total_cmp);
        let within: Vec<f64> = measured.iter().map(|m| m.metrics.overall.within).collect();
        let ssims: Vec<f64> = measured.iter().filter_map(|m| m.metrics.ssim).collect();
        let mut histograms: Vec<f64> = measured.iter().map(|m| m.metrics.distribution.histogram).collect();
        histograms.sort_by(f64::total_cmp);
        let mut spectra: Vec<f64> = measured
            .iter()
            .filter_map(|m| m.metrics.distribution.spectrum)
            .collect();
        spectra.sort_by(f64::total_cmp);
        SubjectSummary {
            histogram_median: median(&histograms),
            spectrum_median: median(&spectra),
            subject,
            measured: measured.len(),
            skipped: count(|o| matches!(o, Outcome::Skipped(_))),
            failed: count(|o| matches!(o, Outcome::Failed(_))),
            mae_mean: mean(&maes),
            mae_median: median(&maes),
            within_mean: mean(&within),
            ssim_mean: mean(&ssims),
        }
    }
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// Median of sorted values.
fn median(v: &[f64]) -> Option<f64> {
    match v.len() {
        0 => None,
        n if n % 2 == 1 => Some(v[n / 2]),
        n => Some(0.5 * (v[n / 2 - 1] + v[n / 2])),
    }
}

/// `12.34`, or `–` for no value.
pub fn fmt_opt(v: Option<f64>, decimals: usize) -> String {
    v.map_or_else(|| "–".to_string(), |v| format!("{v:.decimals$}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn medians_and_means() {
        assert_eq!(median(&[1.0, 2.0, 9.0]), Some(2.0));
        assert_eq!(median(&[1.0, 3.0]), Some(2.0));
        assert_eq!(mean(&[]), None);
        assert_eq!(fmt_opt(None, 2), "–");
        assert_eq!(fmt_opt(Some(1.234), 1), "1.2");
        assert!(SubjectSummary::of(&[]).is_empty());
    }
}
