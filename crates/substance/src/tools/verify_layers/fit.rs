//! [`FitTally`]: identifies Painter blend values by pixels. For every value used in a compared channel, the
//! stack is imported again with the value mapped to each of our modes in turn, and the composite is scored
//! against the cache; the mode that reproduces the cache wins. Several modes reproducing it means the data
//! cannot tell them apart (e.g. any normal-map mode with flat normals).

use super::prepared::PreparedSet;
use crate::model::PainterBlendMode;
use crate::translate::layers::cache_score::Score;
use crate::translate::layers::{BlendTable, ImageSource};
use materialcraft::paint::{BlendMode, Channel};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};

/// Sum of per-channel mean errors.
#[derive(Debug, Clone, Copy, Default)]
struct Sum {
    total: f64,
    channels: usize,
    reproduced: usize,
}

impl Sum {
    fn add(&mut self, s: Score) {
        self.total += s.mean as f64;
        self.channels += 1;
        self.reproduced += usize::from(s.reproduced());
    }

    fn mean(&self) -> f64 {
        self.total / self.channels.max(1) as f64
    }
}

#[derive(Debug, Default)]
struct ValueFit {
    table: Sum,
    modes: BTreeMap<&'static str, Sum>,
    examples: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub struct FitTally {
    values: BTreeMap<u64, ValueFit>,
}

impl FitTally {
    /// Tries every mode for each value used in one compared channel. `before` is the score with the table.
    pub fn fit(
        &mut self,
        set: &PreparedSet,
        channel: Channel,
        values: &BTreeSet<u64>,
        before: Score,
        (table, images): (&BlendTable, &dyn ImageSource),
        example: &str,
    ) {
        for &value in values {
            let entry = self.values.entry(value).or_default();
            entry.table.add(before);
            if entry.examples.len() < 4 {
                entry.examples.insert(example.to_string());
            }
            for mode in BlendMode::ALL.into_iter().filter(|m| *m != BlendMode::Passthrough) {
                let stack = set.import(&table.clone().with_override(value, mode), images);
                if let Some(score) = set.score(&stack, channel) {
                    entry.modes.entry(mode.id()).or_default().add(score);
                }
            }
        }
    }
}

impl fmt::Display for FitTally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.values.is_empty() {
            return writeln!(f, "blend fits: no blend value in a compared channel");
        }
        writeln!(
            f,
            "blend fits (mean 8-bit steps over the channels using the value; reproduced = ≤ 0.5):"
        )?;
        let table = BlendTable::painter();
        for (value, fit) in &self.values {
            let mapping = table.map(PainterBlendMode::from_raw(*value));
            let mut ranked: Vec<(&str, Sum)> = fit.modes.iter().map(|(m, s)| (*m, *s)).collect();
            ranked.sort_by(|a, b| a.1.mean().total_cmp(&b.1.mean()));
            let mut line = String::new();
            for (mode, sum) in ranked.iter().take(5) {
                let _ = write!(line, "{mode} {:.2} ({}/{}), ", sum.mean(), sum.reproduced, sum.channels);
            }
            writeln!(
                f,
                "  value {value} (table: {} {:?}): {} channels, table {:.2} ({} reproduced); best: {}",
                mapping.mode.id(),
                mapping.confidence,
                fit.table.channels,
                fit.table.mean(),
                fit.table.reproduced,
                line.trim_end_matches(", ")
            )?;
            let examples: Vec<&str> = fit.examples.iter().map(String::as_str).collect();
            writeln!(f, "    e.g. {}", examples.join("; "))?;
        }
        Ok(())
    }
}
