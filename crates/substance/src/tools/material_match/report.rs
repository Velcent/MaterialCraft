//! `mcsub match-materials`: the suggestions for every material fill of one project or a folder of them, and
//! how close the best-scored tray material's look is to Painter's result, per channel, over all of them. The
//! numbers say how far a replacement is from Painter's look: it is a replacement, not a reproduction.

use super::{MaterialMatch, MaterialMatches, TrayMaterials};
use crate::inspect::Corpus;
use crate::tools::accuracy::ReplacementAccuracy;
use crate::tools::analyze::Analysis;
use crate::tools::parallel::CorpusPool;
use materialcraft::paint::Channel;
use std::fmt;
use std::path::{Path, PathBuf};

/// Every project's matches.
#[derive(Default)]
pub struct MatchTable {
    pub rows: Vec<(String, MaterialMatches)>,
    pub failures: Vec<(String, String)>,
}

impl MatchTable {
    /// Matches every `.spp` below `root` (or `root` itself) against `tray`, several projects at once on `pool`.
    pub fn of_path(
        root: &Path,
        tray: &TrayMaterials,
        pool: CorpusPool,
        progress: &(dyn Fn(&Path) + Sync),
    ) -> MatchTable {
        let files = match root.is_file() {
            true => vec![root.to_path_buf()],
            false => Corpus::discover(root).files().to_vec(),
        };
        let label = |p: &PathBuf| {
            p.strip_prefix(root)
                .ok()
                .filter(|r| !r.as_os_str().is_empty())
                .unwrap_or(p)
                .display()
                .to_string()
        };
        let results = pool.map(
            &files,
            |p| CorpusPool::estimate(p),
            |p| {
                progress(p);
                let analysis = Analysis::of_project(p, &ReplacementAccuracy::default())?;
                MaterialMatches::measure(p, &analysis, tray)
            },
        );
        let mut table = MatchTable::default();
        for (path, r) in files.iter().zip(results) {
            match r {
                Ok(m) => table.rows.push((label(path), m)),
                Err(e) => table.failures.push((label(path), e)),
            }
        }
        table
    }

    fn matches(&self) -> impl Iterator<Item = (&str, &MaterialMatch)> {
        self.rows
            .iter()
            .flat_map(|(p, m)| m.materials.iter().map(move |x| (p.as_str(), x)))
    }
}

/// Minimum, quartiles, maximum and mean of `values` (sorted here).
fn spread(mut values: Vec<f32>) -> String {
    if values.is_empty() {
        return "–".into();
    }
    values.sort_by(f32::total_cmp);
    let at = |q: f32| {
        values
            .get(((values.len() - 1) as f32 * q).round() as usize)
            .copied()
            .unwrap_or(0.0)
    };
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    format!(
        "n {:>4}  min {:.2}  p25 {:.2}  median {:.2}  p75 {:.2}  max {:.2}  mean {:.2}",
        values.len(),
        at(0.0),
        at(0.25),
        at(0.5),
        at(0.75),
        at(1.0),
        mean
    )
}

impl fmt::Display for MatchTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (project, m) in self.matches() {
            let best = m.best();
            let channels = best
                .and_then(|b| b.look.as_ref())
                .map_or("look not measured".to_string(), |l| {
                    l.channels
                        .iter()
                        .map(|(c, v)| format!("{} {v:.2}", c.id()))
                        .collect::<Vec<_>>()
                        .join(" ")
                });
            writeln!(
                f,
                "{project} / {}: {} uses ({} shown) → {} score {:.2} (name {:.2}); {channels}",
                m.name,
                m.uses,
                m.visible_uses,
                best.map_or("–", |b| b.reference.as_str()),
                best.map_or(0.0, |b| b.score),
                best.map_or(0.0, |b| b.name_score),
            )?;
        }
        let all: Vec<&MaterialMatch> = self.matches().map(|(_, m)| m).collect();
        let measured: Vec<&MaterialMatch> = all.iter().copied().filter(|m| m.painter.is_some()).collect();
        let named = all
            .iter()
            .filter(|m| m.best().is_some_and(|b| b.name_score > 0.0))
            .count();
        let mut distinct: Vec<&str> = all.iter().map(|m| m.name.as_str()).collect();
        distinct.sort_unstable();
        distinct.dedup();
        writeln!(
            f,
            "\n{} material fills ({} distinct materials) in {} projects; Painter's look measured for {}; best pick matches by name for {}",
            all.len(),
            distinct.len(),
            self.rows.len(),
            measured.len(),
            named
        )?;
        writeln!(
            f,
            "similarity of the best-scored material's look to Painter's result (0..1, 1 = same statistics):"
        )?;
        let look = |m: &&MaterialMatch| m.best().and_then(|b| b.look.clone());
        writeln!(
            f,
            "  {:<11} {}",
            "overall",
            spread(measured.iter().filter_map(look).map(|l| l.overall).collect())
        )?;
        for channel in [
            Channel::BaseColor,
            Channel::Roughness,
            Channel::Metallic,
            Channel::Height,
        ] {
            let values = measured
                .iter()
                .filter_map(look)
                .filter_map(|l| l.channels.iter().find(|(c, _)| *c == channel).map(|(_, v)| *v))
                .collect();
            writeln!(f, "  {:<11} {}", channel.id(), spread(values))?;
        }
        writeln!(
            f,
            "  {:<11} {}",
            "score",
            spread(all.iter().filter_map(|m| m.best().map(|b| b.score)).collect())
        )?;
        for (project, e) in &self.failures {
            writeln!(f, "FAILED {project}: {e}")?;
        }
        Ok(())
    }
}
