//! `mcperf`: runs MaterialCraft's Paint mode performance scenarios and keeps their history.
//!
//! ```text
//! mcperf list [--json]                       the scenarios (built-in and from scenario files)
//! mcperf run <id|id,id|all> [--repeat N] [--json] [--no-history] [--threshold PCT]
//! mcperf compare [<id|all>] [--threshold PCT]  the last run of each scenario against the one before
//! ```
//!
//! `run` plays each scenario in the editor binary (`materialcraft-gui`, found next to `mcperf`; build both in
//! release), appends one JSON line per scenario to the history (`$MC_PERF_HISTORY`, else
//! `MaterialCraft-docs/perf/history.jsonl`) and compares it with the previous run on this machine. Scenarios
//! whose corpus files are missing are skipped with a note.
//!
//! Benchmarks are heavy jobs: run them one at a time, under the shared lock and at low priority:
//!
//! ```text
//! cargo build --release -p materialcraft-editor -p materialcraft-perf
//! flock ../Corpus/.heavy.lock nice -n 10 target/release/mcperf run all
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    match materialcraft_perf::cli::main(std::env::args().skip(1).collect()) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        Err(e) => {
            eprintln!("mcperf: {e}");
            ExitCode::FAILURE
        }
    }
}
