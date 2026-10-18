//! (a) A paint stroke on a new project: a project made from a mesh at 2K (File > New project), the paint tool on its
//! paint layer, a stroke of 200 pointer samples across the view, two per frame. Now the built-in scenario
//! `stroke-2k`, played by the [`runner`] (`stroke_play` holds the frame loop); `mcperf run stroke-2k` runs the
//! same and keeps its history.

use super::{corpus_file, runner};
use materialcraft_perf::{Source, Status, builtin};

#[test]
#[ignore]
fn perf_stroke_on_new_project() {
    let mut s = builtin::scenarios().into_iter().find(|s| s.id == builtin::STROKE).expect("the built-in stroke scenario");
    let Some(mesh) = corpus_file("MC_PERF_MESH", &s.source.path().to_string_lossy()) else { return };
    if let Source::NewProject { mesh: m, .. } = &mut s.source {
        *m = mesh;
    }
    let r = runner::run(&s);
    eprint!("{}", r.table());
    assert_eq!(r.status, Status::Measured);
    assert!(r.get("dabs").unwrap_or(0.0) > 0.0, "the stroke painted nothing");
}
