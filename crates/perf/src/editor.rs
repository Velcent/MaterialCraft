//! The hand-over between `mcperf` and the runner in the editor binary. `mcperf` writes the scenario to a file and
//! starts `materialcraft-gui --perf-run <scenario.json> --out <report.json>`; the editor plays it headless (no
//! window), writes the [`RunReport`] and exits. One process per scenario, so every run starts with a fresh heap and
//! fresh worker threads.

use crate::report::RunReport;
use crate::scenario::{Action, Scenario};
use materialcraft::json;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The editor's flag that runs one scenario instead of opening the window.
pub const RUN_FLAG: &str = "--perf-run";
/// Overrides where the editor binary is found.
pub const EDITOR_ENV: &str = "MC_PERF_EDITOR";
const EDITOR_NAME: &str = "materialcraft-gui";

/// `$MC_PERF_EDITOR`, else `materialcraft-gui` next to the running program (cargo puts both in `target/<profile>`).
pub fn editor_binary() -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os(EDITOR_ENV) {
        return Ok(PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| format!("can't find this program: {e}"))?;
    let path = exe.with_file_name(format!("{EDITOR_NAME}{}", std::env::consts::EXE_SUFFIX));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("{} not found; build it with `cargo build --release -p materialcraft-editor` or set {EDITOR_ENV}", path.display()))
    }
}

/// Why `s` can't run here (a file it reads is missing), if so.
pub fn missing_input(s: &Scenario) -> Option<String> {
    let mut files = vec![s.source.path()];
    if let Action::Stroke(spec) = &s.action
        && let Some(p) = &spec.preset
    {
        files.push(p);
    }
    files.into_iter().find(|p| !p.exists()).map(|p| format!("{} not found", p.display()))
}

/// Plays `s` in the editor at `editor`; `scratch` holds the two hand-over files.
pub fn run_in(editor: &Path, s: &Scenario, scratch: &Path) -> RunReport {
    if let Some(why) = missing_input(s) {
        return RunReport::skipped(&s.id, why);
    }
    let (input, output) = (scratch.join(format!("{}.scenario.json", s.id)), scratch.join(format!("{}.report.json", s.id)));
    let _ = std::fs::remove_file(&output);
    if let Err(e) = std::fs::create_dir_all(scratch).and_then(|()| std::fs::write(&input, s.to_json().to_string_pretty())) {
        return RunReport::failed(&s.id, format!("can't write {}: {e}", input.display()));
    }
    let status = Command::new(editor).arg(RUN_FLAG).arg(&input).arg("--out").arg(&output).status();
    let report = match (status, std::fs::read_to_string(&output)) {
        (Err(e), _) => RunReport::failed(&s.id, format!("can't start {}: {e}", editor.display())),
        (Ok(_), Ok(text)) => {
            json::parse(&text).map_err(|e| e.to_string()).and_then(|v| RunReport::from_json(&v)).unwrap_or_else(|e| RunReport::failed(&s.id, e))
        }
        (Ok(code), Err(_)) => RunReport::failed(&s.id, format!("the runner ended ({code}) without a report")),
    };
    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output);
    report
}

/// The editor's side: when `args` (without the program name) are `--perf-run <scenario> --out <report>`, runs the
/// scenario with `run`, writes the report and returns the exit code; `None` for any other command line.
pub fn serve(args: &[String], run: impl FnOnce(&Scenario) -> RunReport) -> Option<i32> {
    let (flag, input, out_flag, output) = (args.first()?, args.get(1), args.get(2), args.get(3));
    if flag != RUN_FLAG {
        return None;
    }
    let (Some(input), Some("--out"), Some(output)) = (input, out_flag.map(String::as_str), output) else {
        eprintln!("usage: {EDITOR_NAME} {RUN_FLAG} <scenario.json> --out <report.json>");
        return Some(2);
    };
    let scenario = std::fs::read_to_string(input)
        .map_err(|e| e.to_string())
        .and_then(|t| json::parse(&t).map_err(|e| e.to_string()))
        .and_then(|v| Scenario::from_json(&v));
    let report = match scenario {
        Ok(s) => run(&s),
        Err(e) => RunReport::failed("unknown", format!("{input}: {e}")),
    };
    match std::fs::write(output, report.to_json().to_string_pretty()) {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("can't write {output}: {e}");
            Some(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtin;

    #[test]
    fn serve_reads_the_scenario_and_writes_the_report() {
        let dir = std::env::temp_dir().join(format!("mcperf-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let s = builtin::scenarios().remove(0);
        let (input, output) = (dir.join("s.json"), dir.join("r.json"));
        std::fs::write(&input, s.to_json().to_string()).unwrap();
        let args: Vec<String> = [RUN_FLAG, input.to_str().unwrap(), "--out", output.to_str().unwrap()].iter().map(|a| a.to_string()).collect();
        let code = serve(&args, |got| {
            let mut r = RunReport::new(&got.id);
            r.put("open_ms", 1.0);
            r
        });
        assert_eq!(code, Some(0));
        let r = RunReport::from_json(&json::parse(&std::fs::read_to_string(&output).unwrap()).unwrap()).unwrap();
        assert_eq!((r.scenario.as_str(), r.get("open_ms")), (s.id.as_str(), Some(1.0)));
        assert_eq!(serve(&["--other".to_string()], |_| RunReport::new("never run")), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
