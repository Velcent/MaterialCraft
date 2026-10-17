//! `mcparity` commands:
//!
//! ```text
//! mcparity list                                   the subjects
//! mcparity cases <subject> [--projects …] [--corpus …]
//!                                                 which projects/sets can run, and why others cannot
//! mcparity run <subject|all> [--projects <file|glob|list>]… [--corpus <dir>]… [--limit N]
//!              [--max-size N] [--jobs N] [--out <dir>] [--history <file> | --no-history] [--open]
//!              [--cache-cases final|isolated|both]
//! ```
//!
//! `--projects`/`--corpus` may repeat; without them `$MC_SUBSTANCE_CORPUS` is used. `run` prints a table,
//! writes the HTML report to `$XDG_CACHE_HOME/materialcraft-parity/<run>/index.html` (or `--out`) and appends
//! the numbers to the history (`$MC_PARITY_HISTORY`, else the docs' `parity/history.jsonl`).
//!
//! `generator.<id>` subjects measure MaterialCraft's replacements for Substance procedurals against Painter's
//! texture cache (channels where that generator is the only replaced procedural); `mcsub analyze` and the
//! import review read their median MAE from the history. `texture.<id>` subjects measure the procedural
//! textures the same way; for those look-alikes read the histogram and spectrum distances, not the MAE.
//! `filter.<id>` subjects measure the filters the same way (mask filters over content the import reproduces).
//! Both also get *isolated* cases (`--cache-cases`, default both): the effect's own output read back from the
//! cache through the rest of the stack, also in channels with other unknown layers, on the texels they do not
//! reach (`materialcraft_substance::tools::verify_layers::isolate`).

mod args;

pub use args::{Args, DEFAULT_MAX_SIZE};

use crate::history::{History, RunStamp};
use crate::provenance;
use crate::report::run_dir::iso_time;
use crate::report::{HtmlReport, RunDir, text};
use crate::run::survey::{reason_counts, survey};
use crate::run::{RunOptions, Runner};
use crate::subjects;
use std::time::SystemTime;

pub const USAGE: &str = "usage: mcparity list | cases <subject> | run <subject|all> [options] (see `mcparity help`)";

pub fn run(raw: Vec<String>) -> Result<(), String> {
    let args = Args::parse(raw)?;
    match args.command.as_str() {
        "list" => {
            list();
            Ok(())
        }
        "cases" => cases(&args),
        "run" => run_cases(&args),
        "help" | "--help" | "-h" => {
            println!("{}", HELP);
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n{USAGE}")),
    }
}

const HELP: &str = "mcparity list
mcparity cases <subject> [--projects <file|glob|list>]... [--corpus <dir>]...
mcparity run <subject|all> [--projects ...]... [--corpus <dir>]... [--limit N] [--max-size N]
             [--jobs N (projects at once; default a quarter of the cores, within 60 % of free memory)] [--out <dir>] [--history <file> | --no-history] [--open]
             [--cache-cases final|isolated|both (generator/texture cases: final look, isolated read-back, or both; default both)]";

fn list() {
    for s in subjects::all() {
        println!(
            "{:<20} {}  (against Painter's {})",
            s.id(),
            s.description(),
            s.reference()
        );
    }
}

fn selected(args: &Args) -> Result<Vec<Box<dyn crate::subject::Subject>>, String> {
    let selector = args.words.first().ok_or_else(|| format!("which subject?\n{USAGE}"))?;
    let chosen = subjects::select(selector);
    if chosen.is_empty() {
        return Err(format!("no subject matches '{selector}' (see `mcparity list`)"));
    }
    Ok(chosen)
}

fn cases(args: &Args) -> Result<(), String> {
    let subject = selected(args)?.into_iter().next().ok_or("no subject")?;
    if subject.generator().is_some() {
        return Err(format!(
            "{}: generator cases are found while measuring; use `mcparity run {}`",
            subject.id(),
            subject.id()
        ));
    }
    let projects = args.projects.resolve()?;
    let rows = survey(subject.as_ref(), &projects, &mut |p| eprintln!("reading {p}"));
    for r in &rows {
        let status = match &r.status {
            Ok(high) => format!("ok   high: {high}"),
            Err(why) => format!("no   {why}"),
        };
        println!("{} / {}: {status}", r.project.display(), r.set);
    }
    let ok = rows.iter().filter(|r| r.status.is_ok()).count();
    println!(
        "\n{}: {ok} of {} texture sets in {} projects can run",
        subject.id(),
        rows.len(),
        projects.len()
    );
    for (reason, n) in reason_counts(&rows) {
        println!("  {n:>4} × {reason}");
    }
    Ok(())
}

fn run_cases(args: &Args) -> Result<(), String> {
    let chosen = selected(args)?;
    let projects = args.projects.resolve()?;
    let started = SystemTime::now();
    let dir = RunDir::create(args.out.as_deref()).map_err(|e| format!("run folder: {e}"))?;
    let mut report = HtmlReport::new(dir);
    let runner = Runner {
        subjects: &chosen,
        options: RunOptions {
            limit: args.limit,
            max_size: args.max_size,
            jobs: args.jobs,
            cache_cases: args.cache_cases,
        },
    };
    let results = runner.run(&projects, &mut report);
    report.order_like(&results);
    println!("{}", text::table(&results));
    report.problems.iter().for_each(|p| eprintln!("{p}"));
    let (time, commit) = (iso_time(started), provenance::commit());
    let page = report
        .finish(&time, &commit, projects.len())
        .map_err(|e| format!("report: {e}"))?;
    println!("report: {}", page.display());
    if args.no_history {
        println!("history: off (--no-history)");
    } else {
        write_history(args, report.dir().id.as_str(), &time, &commit, &results)?;
    }
    if args.open {
        let _ = std::process::Command::new("xdg-open").arg(&page).spawn();
    }
    Ok(())
}

fn write_history(
    args: &Args,
    run: &str,
    time: &str,
    commit: &str,
    results: &[crate::run::CaseResult],
) -> Result<(), String> {
    let history = match &args.history {
        Some(p) => Some(History::at(p)),
        None => History::default_location(),
    };
    let Some(h) = history else {
        println!("history: not written (no docs folder found; use --history <file>)");
        return Ok(());
    };
    let stamp = RunStamp {
        run,
        time,
        commit,
        max_size: args.max_size,
    };
    h.append(&stamp, results)
        .map_err(|e| format!("history {}: {e}", h.path.display()))?;
    println!("history: {}", h.path.display());
    Ok(())
}
