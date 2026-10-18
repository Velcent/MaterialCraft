//! (b) Single edits on big projects: each document is opened (every set composited), then one layer edit at a time
//! goes through the layer panel's path ([`stack_editor::apply`], then the recomposite of the sets it reaches) and
//! the time to the updated maps is measured: opacity, visibility, blend mode, a mask edit, an effect parameter and
//! an undo (which goes through the history's path). Now the built-in scenario `edits-big`, played by the
//! [`runner`] (`edit_play` holds the edits) on each document of `MC_PERF_DOCS`.
//!
//! [`stack_editor::apply`]: crate::gui_paint::stack_editor::apply

use super::runner;
use materialcraft_perf::{Source, Status, builtin};
use std::path::PathBuf;

#[test]
#[ignore]
fn perf_layer_edits() {
    let scenario = builtin::scenarios().into_iter().find(|s| s.id == builtin::EDITS).expect("the built-in edit scenario");
    let default = scenario.source.path().display().to_string();
    let list = std::env::var("MC_PERF_DOCS").unwrap_or(default);
    for path in list.split(':').filter(|p| !p.is_empty()).map(PathBuf::from) {
        let s = materialcraft_perf::Scenario { source: Source::Document(path), ..scenario.clone() };
        let r = runner::run(&s);
        eprint!("{}: {}", s.source.path().display(), r.table());
        assert!(!matches!(r.status, Status::Failed(_)), "{:?}", r.status);
    }
}
