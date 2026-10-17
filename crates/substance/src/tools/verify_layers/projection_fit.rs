//! [`ProjectionFitTally`]: fits Painter's volume projection convention by pixels (requirements/projections.md
//! §2.3, PJ-30). For every compared channel whose status names an unverified volume convention, the stack is
//! imported again with each candidate [`VolumeConvention`] (the importer's, then one setting changed at a time:
//! position unit, size unit, Euler order, blend-hardness curve, image flips) and scored against the cache. The
//! candidates are the same for every channel, so their sums per mode group compare directly.

use super::prepared::PreparedSet;
use crate::translate::layers::cache_score::Score;
use crate::translate::layers::{BlendTable, HardnessCurve, ImageSource, ProjectionTable, VolumeConvention};
use materialcraft::paint::Channel;
use materialcraft::projection::EulerOrder;
use std::collections::BTreeMap;
use std::fmt::{self, Write};

/// The mode names the status reasons mention ("tri-planar projection: Painter's volume convention …").
const MARKER: &str = " projection: Painter's volume convention not verified";

/// The volume modes a channel's status names, joined ("planar+tri-planar"); `None` without any.
pub fn modes_in(reasons: &[String]) -> Option<String> {
    let mut modes: Vec<&str> = reasons
        .iter()
        .filter_map(|r| r.strip_suffix(" by pixels")?.strip_suffix(MARKER))
        .filter(|m| !m.contains(':'))
        .collect();
    modes.sort_unstable();
    modes.dedup();
    (!modes.is_empty()).then(|| modes.join("+"))
}

/// The importer's convention first, then each setting changed on its own.
pub fn candidates() -> Vec<VolumeConvention> {
    let base = VolumeConvention::PAINTER;
    let mut out = vec![base];
    let mut push = |c: VolumeConvention| {
        if !out.contains(&c) {
            out.push(c);
        }
    };
    for position in [0.25, 1.0] {
        push(VolumeConvention { position, ..base });
    }
    for size in [0.5, 2.0] {
        push(VolumeConvention { size, ..base });
    }
    for euler in EulerOrder::ALL {
        push(VolumeConvention { euler, ..base });
    }
    for hardness in HardnessCurve::ALL {
        push(VolumeConvention { hardness, ..base });
    }
    for flip in [[true, false], [false, true], [true, true]] {
        push(VolumeConvention { flip, ..base });
    }
    out
}

#[derive(Debug, Clone, Copy, Default)]
struct Sum {
    total: f64,
    channels: usize,
    reproduced: usize,
}

impl Sum {
    fn add(&mut self, s: Score) {
        self.total += f64::from(s.mean);
        self.channels += 1;
        self.reproduced += usize::from(s.reproduced());
    }

    fn mean(&self) -> f64 {
        self.total / self.channels.max(1) as f64
    }
}

/// The scores of one channel under every candidate (index = [`candidates`] order).
#[derive(Debug)]
pub struct ProjectionFitRecord {
    modes: String,
    scores: Vec<Option<Score>>,
    example: String,
}

impl ProjectionFitRecord {
    pub fn measure(
        set: &PreparedSet,
        channel: Channel,
        modes: String,
        (table, images): (&BlendTable, &dyn ImageSource),
        example: &str,
    ) -> ProjectionFitRecord {
        let scores = candidates()
            .into_iter()
            .map(|c| {
                let stack = set.import_with(table, &ProjectionTable::with_volume(c), images);
                set.score(&stack, channel)
            })
            .collect();
        ProjectionFitRecord {
            modes,
            scores,
            example: example.to_string(),
        }
    }
}

#[derive(Debug, Default)]
struct GroupFit {
    candidates: BTreeMap<usize, Sum>,
    examples: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ProjectionFitTally {
    groups: BTreeMap<String, GroupFit>,
}

impl ProjectionFitTally {
    pub fn add(&mut self, records: impl IntoIterator<Item = ProjectionFitRecord>) {
        for r in records {
            let g = self.groups.entry(r.modes).or_default();
            if g.examples.len() < 4 {
                g.examples.push(r.example);
            }
            for (i, s) in r.scores.into_iter().enumerate() {
                if let Some(s) = s {
                    g.candidates.entry(i).or_default().add(s);
                }
            }
        }
    }
}

impl fmt::Display for ProjectionFitTally {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.groups.is_empty() {
            return writeln!(f, "projection fits: no volume projection in a compared channel");
        }
        writeln!(
            f,
            "projection fits (mean 8-bit steps over the channels; reproduced = ≤ 0.5):"
        )?;
        let all = candidates();
        for (modes, g) in &self.groups {
            let label = |i: usize| all.get(i).map_or_else(|| "?".to_string(), VolumeConvention::label);
            let importer = g.candidates.get(&0).copied().unwrap_or_default();
            let mut ranked: Vec<(usize, Sum)> = g.candidates.iter().map(|(i, s)| (*i, *s)).collect();
            ranked.sort_by(|a, b| a.1.mean().total_cmp(&b.1.mean()));
            let mut line = String::new();
            for (i, s) in ranked.iter().take(4) {
                let _ = write!(
                    line,
                    "{} {:.2} ({}/{}); ",
                    label(*i),
                    s.mean(),
                    s.reproduced,
                    s.channels
                );
            }
            writeln!(
                f,
                "  {modes}: {} channels, importer ({}) {:.2} ({} reproduced); best: {}",
                importer.channels,
                label(0),
                importer.mean(),
                importer.reproduced,
                line.trim_end_matches("; ")
            )?;
            writeln!(f, "    e.g. {}", g.examples.join("; "))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasons_name_the_modes_and_candidates_start_with_the_importer() {
        let reasons = vec![
            "tri-planar projection: Painter's volume convention not verified by pixels".to_string(),
            "something else".to_string(),
            "planar projection: Painter's volume convention not verified by pixels".to_string(),
        ];
        assert_eq!(modes_in(&reasons).as_deref(), Some("planar+tri-planar"));
        assert_eq!(modes_in(&["x".to_string()]), None);
        let c = candidates();
        assert_eq!(c[0], VolumeConvention::PAINTER);
        assert!(c.len() > 10);
    }
}
