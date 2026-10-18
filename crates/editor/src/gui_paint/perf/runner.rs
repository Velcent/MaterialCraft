//! The one runner: plays any [`Scenario`] through the Paint screen's real paths and returns its [`RunReport`].
//! Opening is [`source::open`]; the action is [`stroke_play`], [`edit_play`] or another full recomposite.

use super::measure::{Recomposite, put_all};
use super::{edit_play, source, stroke_play};
use materialcraft_perf::editor::missing_input;
use materialcraft_perf::{Action, RunReport, Scenario, Source, Status};

/// Plays `s`: skipped when a file it reads is missing, failed (with the reason) when it can't be played.
pub fn run(s: &Scenario) -> RunReport {
    if let Some(why) = missing_input(s) {
        return RunReport::skipped(&s.id, why);
    }
    let mut r = RunReport::new(&s.id);
    if let Err(e) = play(s, &mut r) {
        r.status = Status::Failed(e);
    }
    r
}

fn play(s: &Scenario, r: &mut RunReport) -> Result<(), String> {
    let mut b = source::open(&s.source, r)?;
    let repeat = s.repeat.max(1);
    match &s.action {
        Action::Open => {
            let list: Vec<Recomposite> = (0..repeat).map(|_| Recomposite::rebuild(&mut b)).collect();
            put_all(r, &list);
            Ok(())
        }
        Action::Stroke(spec) => stroke_play::play(&mut b, spec, repeat, matches!(s.source, Source::NewProject { .. }), r),
        Action::Edits(list) => edit_play::play(&mut b, list, repeat, r),
    }
}

/// The editor's `--perf-run` entry (see `materialcraft_perf::editor`): `Some(exit code)` when the command line
/// asks for it.
pub fn from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    materialcraft_perf::editor::serve(&args, run)
}
