//! [`Census`]: how many cases each effect id gets, by status, and which roadmap step would turn the others
//! into strict cases (`mcsub isolate`). Measured numbers (texels solved) are added when cases were measured.

use super::measure::TexelStats;
use super::plan::{CasePlan, CaseStatus, Unblocker};
use super::slots::Family;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Cases measured with at least this many solved texels count as measurable on the corpus.
pub const MIN_SOLVED: usize = 1000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tally {
    pub by_status: BTreeMap<CaseStatus, usize>,
    /// Non-strict cases that only this step stands between (strict once it is done).
    pub only_step: BTreeMap<Unblocker, usize>,
    /// Non-strict cases needing several steps.
    pub several_steps: usize,
    /// Measured cases and those with at least [`MIN_SOLVED`] solved texels.
    pub measured: usize,
    pub usable: usize,
    pub solved_texels: usize,
}

/// Per (family, id).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Census {
    pub rows: BTreeMap<(Family, String), Tally>,
}

impl Census {
    pub fn add(&mut self, plan: &CasePlan) {
        let tally = self
            .rows
            .entry((plan.candidate.family, plan.candidate.id()))
            .or_default();
        *tally.by_status.entry(plan.status).or_default() += 1;
        if plan.status == CaseStatus::Strict {
            return;
        }
        let steps = plan.unblockers();
        match steps.len() {
            1 => {
                *tally
                    .only_step
                    .entry(*steps.iter().next().unwrap_or(&Unblocker::Other))
                    .or_default() += 1
            }
            _ => tally.several_steps += 1,
        }
    }

    /// A measured case of `plan`.
    pub fn measured(&mut self, plan: &CasePlan, stats: &TexelStats) {
        let tally = self
            .rows
            .entry((plan.candidate.family, plan.candidate.id()))
            .or_default();
        tally.measured += 1;
        tally.solved_texels += stats.solved;
        tally.usable += usize::from(stats.solved >= MIN_SOLVED);
    }

    pub fn merge(&mut self, other: Census) {
        for (key, t) in other.rows {
            let mine = self.rows.entry(key).or_default();
            for (s, n) in t.by_status {
                *mine.by_status.entry(s).or_default() += n;
            }
            for (s, n) in t.only_step {
                *mine.only_step.entry(s).or_default() += n;
            }
            mine.several_steps += t.several_steps;
            mine.measured += t.measured;
            mine.usable += t.usable;
            mine.solved_texels += t.solved_texels;
        }
    }

    /// One row per id (most cases first within a family), then totals per family.
    pub fn table(&self) -> String {
        let statuses = [
            CaseStatus::Strict,
            CaseStatus::WithNuisances,
            CaseStatus::TooManyNuisances,
            CaseStatus::SameLayer,
            CaseStatus::FilterInput,
            CaseStatus::Blocked,
        ];
        let steps = [
            Unblocker::StrokeReplay,
            Unblocker::NativeEffects,
            Unblocker::Filters,
            Unblocker::Projections,
            Unblocker::Procedurals,
            Unblocker::Other,
        ];
        let mut out = String::new();
        let _ = writeln!(
            out,
            "family\tid\tstrict\twith-nuisances\ttoo-many\tsame-layer\tfilter-input\tblocked\tmeasured\tusable(>={MIN_SOLVED})\tsolved-texels\tonly-32\tonly-13\tonly-22\tonly-24\tonly-procedurals\tonly-other\tseveral"
        );
        let mut rows: Vec<(&(Family, String), &Tally)> = self.rows.iter().collect();
        let total = |t: &Tally| t.by_status.values().sum::<usize>();
        rows.sort_by(|a, b| {
            a.0.0
                .cmp(&b.0.0)
                .then(total(b.1).cmp(&total(a.1)))
                .then(a.0.1.cmp(&b.0.1))
        });
        for ((family, id), t) in rows {
            let mut line = format!("{}\t{id}", family.id());
            for s in statuses {
                let _ = write!(line, "\t{}", t.by_status.get(&s).copied().unwrap_or(0));
            }
            let _ = write!(line, "\t{}\t{}\t{}", t.measured, t.usable, t.solved_texels);
            for s in steps {
                let _ = write!(line, "\t{}", t.only_step.get(&s).copied().unwrap_or(0));
            }
            let _ = writeln!(out, "{line}\t{}", t.several_steps);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::verify_layers::isolate::slots::{
        Candidate, EffectAddress, LayerAddress, Nuisance, NuisanceKind, Place,
    };
    use materialcraft::paint::Channel;

    fn plan(status: CaseStatus, nuisances: Vec<(Nuisance, NuisanceKind)>) -> CasePlan {
        let address = EffectAddress {
            layer: LayerAddress(vec![1]),
            place: Place::Mask(0),
        };
        CasePlan {
            channel: Channel::Roughness,
            candidate: Candidate {
                address,
                graph: "mg_dirt".into(),
                ours: Some("dirt"),
                family: Family::Generator,
                path: "Dirt".into(),
                projection: 0,
            },
            nuisances,
            blockers: Vec::new(),
            status,
        }
    }

    #[test]
    fn tallies_status_and_the_step_that_unblocks() {
        let strokes = (Nuisance::Layer(LayerAddress(vec![2])), NuisanceKind::Strokes);
        let levels = (Nuisance::Layer(LayerAddress(vec![3])), NuisanceKind::Levels);
        let mut c = Census::default();
        c.add(&plan(CaseStatus::Strict, Vec::new()));
        c.add(&plan(CaseStatus::WithNuisances, vec![strokes.clone()]));
        c.add(&plan(CaseStatus::WithNuisances, vec![strokes, levels]));
        c.measured(
            &plan(CaseStatus::Strict, Vec::new()),
            &TexelStats {
                solved: 5000,
                ..Default::default()
            },
        );
        let mut all = Census::default();
        all.merge(c.clone());
        all.merge(c);
        let t = &all.rows[&(Family::Generator, "dirt".to_string())];
        assert_eq!(t.by_status[&CaseStatus::Strict], 2);
        assert_eq!(t.by_status[&CaseStatus::WithNuisances], 4);
        assert_eq!(t.only_step[&Unblocker::StrokeReplay], 2);
        assert_eq!((t.several_steps, t.measured, t.usable), (2, 2, 2));
        assert!(all.table().lines().nth(1).unwrap().starts_with("generator\tdirt\t2\t4"));
    }
}
