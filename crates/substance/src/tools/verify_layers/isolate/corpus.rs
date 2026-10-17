//! [`census_files`]: the [`Census`] of many projects (`mcsub isolate`), several at once on a [`CorpusPool`],
//! with one line per case. With `measure`, every measurable case is composited (Painter's side only) and its
//! texel counts recorded.

use super::cases::{IsolateOptions, isolated_cases, project_plans};
use super::census::Census;
use super::measure::TexelStats;
use super::plan::CasePlan;
use crate::tools::parallel::CorpusPool;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct CorpusCensus {
    pub census: Census,
    /// One tab-separated line per case (see [`CASE_HEADER`]).
    pub cases: Vec<String>,
    pub failures: Vec<(String, String)>,
}

pub const CASE_HEADER: &str = "project\tset\tchannel\tfamily\tid\tlayer\tin-mask\tprojection\tstatus\tnuisances\tsteps\tblockers\tisland\tclean\tsolved\toutside\tcomposites";

pub fn census_files(
    files: &[PathBuf],
    measure: bool,
    options: &IsolateOptions,
    pool: CorpusPool,
    progress: &(dyn Fn(&Path) + Sync),
) -> CorpusCensus {
    let results = pool.map(
        files,
        |p| CorpusPool::estimate(p),
        |p| {
            progress(p);
            project(p, measure, options)
        },
    );
    let mut out = CorpusCensus::default();
    for (path, r) in files.iter().zip(results) {
        match r {
            Ok(c) => {
                out.census.merge(c.census);
                out.cases.extend(c.cases);
            }
            Err(e) => out.failures.push((path.display().to_string(), e)),
        }
    }
    out
}

fn project(path: &Path, measure: bool, options: &IsolateOptions) -> Result<CorpusCensus, String> {
    let name = path
        .file_name()
        .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let mut out = CorpusCensus::default();
    let mut measured: Vec<(String, CasePlan, TexelStats)> = Vec::new();
    if measure {
        let options = IsolateOptions {
            with_ours: false,
            ..options.clone()
        };
        isolated_cases(path, &|_| true, &options, &mut |case| {
            measured.push((case.set, case.plan, case.measurement.stats));
        })?;
    }
    for set in project_plans(path)? {
        for plan in &set.plans {
            out.census.add(plan);
            let stats = measured
                .iter()
                .find(|(s, p, _)| *s == set.set && p == plan)
                .map(|(_, _, st)| *st);
            if let Some(stats) = &stats {
                out.census.measured(plan, stats);
            }
            out.cases.push(case_line(&name, &set.set, plan, stats.as_ref()));
        }
    }
    Ok(out)
}

fn case_line(project: &str, set: &str, plan: &CasePlan, stats: Option<&TexelStats>) -> String {
    let c = &plan.candidate;
    let nuisances: Vec<String> = plan.nuisances.iter().map(|(_, k)| format!("{k:?}")).collect();
    let steps: Vec<&str> = plan.unblockers().iter().map(|u| u.label()).collect();
    let numbers = stats.map_or_else(
        || "\t\t\t\t".to_string(),
        |s| {
            format!(
                "{}\t{}\t{}\t{}\t{}",
                s.island, s.clean, s.solved, s.outside, s.composites
            )
        },
    );
    format!(
        "{project}\t{set}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{numbers}",
        plan.channel.id(),
        c.family.id(),
        c.id(),
        c.path,
        c.in_mask(),
        c.projection,
        plan.status.label(),
        nuisances.join(","),
        steps.join(","),
        plan.blockers.join(" | ").replace('\t', " "),
    )
}

impl fmt::Display for CorpusCensus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.census.table())?;
        for (path, e) in &self.failures {
            writeln!(f, "failed\t{path}\t{e}")?;
        }
        Ok(())
    }
}
