//! The `mcperf` command line (see the binary's doc for usage).

use crate::catalog::Catalog;
use crate::compare::{self, THRESHOLD};
use crate::editor;
use crate::history::{History, Record};
use crate::machine::Machine;
use crate::report::{Status, format_value};
use materialcraft::json::Value;
use std::path::PathBuf;
use std::time::SystemTime;

pub const USAGE: &str = "usage:
  mcperf list [--json]
  mcperf run <id|id,id|all> [--repeat N] [--json] [--no-history] [--threshold PCT]
  mcperf compare [<id|all>] [--threshold PCT]
common options: --scenarios <file.json> (more scenarios; repeatable), --history <file.jsonl>
run it under the heavy lock, at low priority:
  flock ../Corpus/.heavy.lock nice -n 10 target/release/mcperf run all";

/// The parsed command line.
#[derive(Debug, Default)]
struct Options {
    command: String,
    target: Option<String>,
    repeat: Option<u32>,
    json: bool,
    no_history: bool,
    threshold: Option<f64>,
    scenarios: Vec<PathBuf>,
    history: Option<PathBuf>,
}

fn parse(args: Vec<String>) -> Result<Options, String> {
    let mut o = Options::default();
    let mut it = args.into_iter();
    o.command = it.next().ok_or(USAGE)?;
    while let Some(a) = it.next() {
        let mut value = |what: &str| it.next().ok_or_else(|| format!("{what} needs a value\n{USAGE}"));
        match a.as_str() {
            "--repeat" => o.repeat = Some(value("--repeat")?.parse().ok().filter(|n| *n > 0).ok_or("--repeat needs a positive whole number")?),
            "--threshold" => {
                o.threshold = Some(value("--threshold")?.parse::<f64>().ok().filter(|t| *t >= 0.0).ok_or("--threshold needs a percentage")? / 100.0)
            }
            "--scenarios" => o.scenarios.push(PathBuf::from(value("--scenarios")?)),
            "--history" => o.history = Some(PathBuf::from(value("--history")?)),
            "--json" => o.json = true,
            "--no-history" => o.no_history = true,
            "-h" | "--help" => o.command = "help".into(),
            _ if !a.starts_with('-') && o.target.is_none() => o.target = Some(a),
            _ => return Err(format!("unknown argument {a:?}\n{USAGE}")),
        }
    }
    Ok(o)
}

/// Runs the command line `args` (without the program name); returns the exit code.
pub fn main(args: Vec<String>) -> Result<i32, String> {
    let o = parse(args)?;
    match o.command.as_str() {
        "list" => list(&o),
        "run" => run(&o),
        "compare" => compare_cmd(&o),
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            Ok(0)
        }
        other => Err(format!("unknown command {other:?}\n{USAGE}")),
    }
}

fn history(o: &Options) -> Option<History> {
    o.history.as_deref().map(History::at).or_else(History::default_location)
}

fn list(o: &Options) -> Result<i32, String> {
    let c = Catalog::load(&o.scenarios)?;
    if o.json {
        println!("{}", Value::Array(c.scenarios.iter().map(|s| s.to_json()).collect()).to_string_pretty());
        return Ok(0);
    }
    for (s, origin) in c.scenarios.iter().zip(&c.origins) {
        let note = editor::missing_input(s).map_or(String::new(), |m| format!(" (skipped: {m})"));
        let origin = if origin == "built-in" { String::new() } else { format!(" [{origin}]") };
        println!("{:<16} {}{origin}{note}", s.id, s.about);
    }
    Ok(0)
}

fn run(o: &Options) -> Result<i32, String> {
    let catalog = Catalog::load(&o.scenarios)?;
    let mut chosen = catalog.select(o.target.as_deref().ok_or(USAGE)?)?;
    if let Some(n) = o.repeat {
        chosen.iter_mut().for_each(|s| s.repeat = n);
    }
    let editor = editor::editor_binary()?;
    let history = if o.no_history { None } else { history(o) };
    match &history {
        Some(h) => eprintln!("history: {}", h.path.display()),
        None if !o.no_history => eprintln!("history: not written (no docs folder found; use --history <file>)"),
        None => {}
    }
    let (commit, dirty) = commit();
    let machine = Machine::this();
    let scratch = std::env::temp_dir().join(format!("mcperf-{}", std::process::id()));
    let threshold = o.threshold.unwrap_or(THRESHOLD);
    let mut records = Vec::new();
    let mut failed = 0;
    for s in &chosen {
        eprintln!("== {} ({})", s.id, s.about);
        let report = editor::run_in(&editor, s, &scratch);
        let record = Record {
            time: materialcraft_parity::report::run_dir::iso_time(SystemTime::now()),
            commit: commit.clone(),
            dirty,
            machine: machine.clone(),
            repeat: s.repeat,
            report,
        };
        match &record.report.status {
            Status::Skipped(why) => eprintln!("   skipped: {why}"),
            Status::Failed(why) => {
                eprintln!("   FAILED: {why}");
                failed += 1;
            }
            Status::Measured => {}
        }
        if o.json {
            println!("{}", record.to_json());
        } else {
            print!("{}", record.report.table());
        }
        if let (Some(h), false) = (&history, matches!(record.report.status, Status::Skipped(_))) {
            h.append(&record).map_err(|e| format!("{}: {e}", h.path.display()))?;
            if let Ok(all) = h.read()
                && let Some((before, after)) = compare::last_two(&all, &s.id)
                && after.time == record.time
                && !o.json
            {
                print!("{}", compare::compare(before, after, threshold).text());
            }
        }
        records.push(record);
    }
    let _ = std::fs::remove_dir_all(&scratch);
    if !o.json {
        print!("\n{}", summary(&records));
    }
    Ok(if failed > 0 { 1 } else { 0 })
}

/// The run's headline numbers, one row per scenario.
pub fn summary(records: &[Record]) -> String {
    const COLUMNS: [(&str, &str); 7] = [
        ("open", "open_ms"),
        ("stamp/frame", "stamp_frame_ms.mean"),
        ("poll/frame", "poll_ms.mean"),
        ("recomp mean", "recomposite_ms.mean"),
        ("recomp p95", "recomposite_ms.p95"),
        ("recomp max", "recomposite_ms.max"),
        ("upload/recomp", "recomposite.upload_mb"),
    ];
    let mut out = format!("{:<16}", "scenario");
    for (title, _) in COLUMNS {
        out.push_str(&format!(" {title:>13}"));
    }
    out.push('\n');
    for r in records {
        out.push_str(&format!("{:<16}", r.report.scenario));
        match &r.report.status {
            Status::Measured => {
                for (_, key) in COLUMNS {
                    let cell = r.report.get(key).map_or("-".to_string(), |v| format_value(key, v).trim().to_string());
                    out.push_str(&format!(" {cell:>13}"));
                }
            }
            Status::Skipped(why) => out.push_str(&format!(" skipped: {why}")),
            Status::Failed(why) => out.push_str(&format!(" FAILED: {why}")),
        }
        out.push('\n');
    }
    out
}

fn compare_cmd(o: &Options) -> Result<i32, String> {
    let h = history(o).ok_or("no history file (no docs folder found; use --history <file>)")?;
    let records = h.read().map_err(|e| format!("{}: {e}", h.path.display()))?;
    let mut ids: Vec<&str> = Vec::new();
    for r in &records {
        if !ids.contains(&r.report.scenario.as_str()) {
            ids.push(&r.report.scenario);
        }
    }
    if let Some(t) = o.target.as_deref().filter(|t| *t != "all") {
        let wanted: Vec<&str> = t.split(',').collect();
        ids.retain(|id| wanted.contains(id));
    }
    let threshold = o.threshold.unwrap_or(THRESHOLD);
    let mut regressions = 0;
    for id in ids {
        match compare::last_two(&records, id) {
            Some((before, after)) => {
                let c = compare::compare(before, after, threshold);
                regressions += c.regressions();
                print!("{}", c.text());
            }
            None => println!("{id}: only one run on this machine"),
        }
    }
    println!("{regressions} regression(s) over {:.0}%", threshold * 100.0);
    Ok(if regressions > 0 { 1 } else { 0 })
}

/// The checkout's commit and whether tracked files have changes.
fn commit() -> (String, bool) {
    let c = materialcraft_parity::provenance::commit();
    match c.strip_suffix("+dirty") {
        Some(hash) => (hash.to_string(), true),
        None => (c, false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::RunReport;

    #[test]
    fn parses_the_command_line() {
        let args = |s: &str| s.split(' ').map(str::to_string).collect::<Vec<_>>();
        let o = parse(args("run all --repeat 3 --json --threshold 15")).unwrap();
        assert_eq!((o.command.as_str(), o.target.as_deref(), o.repeat, o.json), ("run", Some("all"), Some(3), true));
        assert!((o.threshold.unwrap() - 0.15).abs() < 1e-9);
        assert!(parse(args("run all --repeat 0")).is_err());
        assert!(parse(args("run a b")).is_err());
    }

    #[test]
    fn summary_has_a_row_per_record() {
        let mut report = RunReport::new("stroke-2k");
        report.put("open_ms", 812.0);
        let r = Record { time: "t".into(), commit: "c".into(), dirty: false, machine: Machine::this(), repeat: 1, report };
        let s = summary(&[r.clone(), Record { report: RunReport::skipped("x", "gone"), ..r }]);
        assert_eq!(s.lines().count(), 3);
        assert!(s.contains("812 ms") && s.contains("skipped: gone"));
    }
}
