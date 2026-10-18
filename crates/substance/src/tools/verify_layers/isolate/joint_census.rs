//! [`JointCensus`]: how many candidate masks the joint read-back can measure, per effect id (`mcsub isolate
//! --joint`), with one line per candidate; with `measure` the feasible ones are composited and their solved
//! texels counted, with the MAE of our replacement on them where we have one.

use super::census::MIN_SOLVED;
use super::joint_cases::{JointOptions, joint_cases, joint_project_plans};
use super::joint_plan::{JointPlan, JointStatus};
use super::measure::{Measurement, TexelStats};
use super::slots::Family;
use crate::tools::parallel::CorpusPool;
use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JointTally {
    pub by_status: BTreeMap<JointStatus, usize>,
    /// Feasible plans none of whose single-channel cases was measurable.
    pub new: usize,
    pub measured: usize,
    /// Measured with at least [`MIN_SOLVED`] solved texels; `usable_new` of them new.
    pub usable: usize,
    pub usable_new: usize,
    pub solved_texels: usize,
}

#[derive(Default)]
pub struct JointCensus {
    pub rows: BTreeMap<(Family, String), JointTally>,
    pub cases: Vec<String>,
    pub failures: Vec<(String, String)>,
}

pub const JOINT_CASE_HEADER: &str = "project\tset\tfamily\tid\tlayer\tchannels\tstatus\tunknowns\tother-states\tsingle-measurable\tisland\tclean\tsolved\toutside\tfolded\tcomposites\tmae-ours";

impl JointCensus {
    fn add(&mut self, plan: &JointPlan, stats: Option<&TexelStats>) {
        let t = self
            .rows
            .entry((plan.candidate.family, plan.candidate.id()))
            .or_default();
        *t.by_status.entry(plan.status).or_default() += 1;
        let new = plan.status == JointStatus::Feasible && !plan.single_measurable;
        t.new += usize::from(new);
        if let Some(s) = stats {
            t.measured += 1;
            t.solved_texels += s.solved;
            let usable = s.solved >= MIN_SOLVED;
            t.usable += usize::from(usable);
            t.usable_new += usize::from(usable && new);
        }
    }

    fn merge(&mut self, other: JointCensus) {
        for (key, t) in other.rows {
            let mine = self.rows.entry(key).or_default();
            for (s, n) in t.by_status {
                *mine.by_status.entry(s).or_default() += n;
            }
            mine.new += t.new;
            mine.measured += t.measured;
            mine.usable += t.usable;
            mine.usable_new += t.usable_new;
            mine.solved_texels += t.solved_texels;
        }
        self.cases.extend(other.cases);
        self.failures.extend(other.failures);
    }
}

pub fn joint_census_files(
    files: &[PathBuf],
    measure: bool,
    options: &JointOptions,
    pool: CorpusPool,
    progress: &(dyn Fn(&Path) + Sync),
) -> JointCensus {
    let results = pool.map(
        files,
        |p| CorpusPool::estimate(p),
        |p| {
            progress(p);
            project(p, measure, options)
        },
    );
    let mut out = JointCensus::default();
    for (path, r) in files.iter().zip(results) {
        match r {
            Ok(c) => out.merge(c),
            Err(e) => out.failures.push((path.display().to_string(), e)),
        }
    }
    out
}

fn project(path: &Path, measure: bool, options: &JointOptions) -> Result<JointCensus, String> {
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let mut measured: Vec<(String, JointPlan, TexelStats, Option<f32>)> = Vec::new();
    if measure {
        joint_cases(path, &|_| true, options, &mut |case| {
            let mae = mae_ours(&case.measurement);
            measured.push((case.set, case.plan, case.measurement.stats, mae));
        })?;
    }
    let mut out = JointCensus::default();
    for set in joint_project_plans(path)? {
        for plan in &set.plans {
            let found = measured
                .iter()
                .find(|(s, p, _, _)| *s == set.set && p.candidate == plan.candidate);
            let stats = found.map(|(_, _, st, _)| *st);
            out.add(plan, stats.as_ref());
            let mae = found.and_then(|(_, _, _, m)| *m);
            out.cases.push(case_line(&name, &set.set, plan, stats.as_ref(), mae));
        }
    }
    Ok(out)
}

/// Mean difference of Painter's and our read-back on the compared texels, in 8-bit steps.
fn mae_ours(m: &Measurement) -> Option<f32> {
    let ours = m.ours.as_ref()?;
    let (mut sum, mut n) = (0.0f64, 0usize);
    for ((t, o), keep) in m.theirs.px.iter().zip(&ours.px).zip(&m.mask) {
        if *keep {
            sum += f64::from((t[0] - o[0]).abs());
            n += 1;
        }
    }
    (n > 0).then(|| (sum / n as f64 * 255.0) as f32)
}

fn case_line(project: &str, set: &str, plan: &JointPlan, stats: Option<&TexelStats>, mae: Option<f32>) -> String {
    let c = &plan.candidate;
    let channels: Vec<String> = plan.channels.iter().map(|c| c.id()).collect();
    let numbers = stats.map_or_else(
        || "\t\t\t\t\t".to_string(),
        |s| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                s.island, s.clean, s.solved, s.outside, s.not_monotone, s.composites
            )
        },
    );
    format!(
        "{project}\t{set}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{numbers}\t{}",
        c.family.id(),
        c.id(),
        c.path,
        channels.join("+"),
        plan.status.label(),
        plan.unknowns(),
        plan.states(),
        plan.single_measurable,
        mae.map_or_else(String::new, |m| format!("{m:.2}")),
    )
}

impl fmt::Display for JointCensus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let statuses = [
            JointStatus::Feasible,
            JointStatus::TooManyMasks,
            JointStatus::TooManyStates,
            JointStatus::TooFewChannels,
            JointStatus::NotApplicable,
        ];
        let mut out = String::new();
        let _ = writeln!(
            out,
            "family\tid\tfeasible\ttoo-many-masks\ttoo-many-states\ttoo-few-channels\tnot-applicable\tnew\tmeasured\tusable(>={MIN_SOLVED})\tusable-new\tsolved-texels"
        );
        for ((family, id), t) in &self.rows {
            let mut line = format!("{}\t{id}", family.id());
            for s in statuses {
                let _ = write!(line, "\t{}", t.by_status.get(&s).copied().unwrap_or(0));
            }
            let _ = writeln!(
                out,
                "{line}\t{}\t{}\t{}\t{}\t{}",
                t.new, t.measured, t.usable, t.usable_new, t.solved_texels
            );
        }
        for (path, e) in &self.failures {
            let _ = writeln!(out, "failed\t{path}\t{e}");
        }
        write!(f, "{out}")
    }
}
