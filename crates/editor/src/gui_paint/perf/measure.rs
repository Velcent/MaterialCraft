//! [`Recomposite`]: one measured recomposite (its latency and what the scene did meanwhile), and how a group of
//! them goes into a [`RunReport`].

use super::Bench;
use crate::gui_paint::editing::{Moved, history_move};
use crate::gui_paint::scene_stats::SceneStats;
use materialcraft::paint::edit::EditError;
use materialcraft_perf::RunReport;
use materialcraft_perf::report::{Stat, mb, ms};
use std::time::{Duration, Instant};

/// One recomposite: from asking for it to the maps handed to the GPU queue.
#[derive(Debug, Clone, Copy)]
pub struct Recomposite {
    pub latency: Duration,
    pub did: SceneStats,
}

impl Recomposite {
    /// Recomposites `sets` as an edit does (`SceneSync::refresh`) and waits for the result.
    pub fn refresh(b: &mut Bench, sets: Vec<usize>) -> Recomposite {
        let (latency, did) = b.refresh(sets);
        Recomposite { latency, did }
    }

    /// Rebuilds the whole scene (`SceneSync::rebuild`, as opening or a change of the set list does) and waits.
    pub fn rebuild(b: &mut Bench) -> Recomposite {
        let before = b.scene.stats();
        let started = Instant::now();
        b.scene.rebuild(&mut b.doc, b.view);
        b.settle();
        Recomposite { latency: started.elapsed(), did: super::delta(before, b.scene.stats()) }
    }

    /// An undo or redo through the history's path (`PaintMode::move_in_history`) and the recomposite it asks for.
    pub fn history(
        b: &mut Bench,
        step: impl FnOnce(&mut crate::gui_paint::document_state::OpenDocument) -> Result<(), EditError>,
    ) -> Result<Recomposite, EditError> {
        let before = b.scene.stats();
        let started = Instant::now();
        let (result, moved) = history_move(&mut b.doc, step);
        result?;
        match moved {
            Moved::SetsAddedOrRemoved => b.scene.rebuild(&mut b.doc, b.view),
            Moved::Changed(sets) => b.scene.refresh(&mut b.doc, b.view, sets),
        }
        b.settle();
        Ok(Recomposite { latency: started.elapsed(), did: super::delta(before, b.scene.stats()) })
    }
}

/// Puts `prefix_ms` (latency mean, p95, max) and the means per recomposite of `prefix.composite_ms`,
/// `.prepare_ms`, `.handoff_ms`, `.upload_mb` and `.sets`; nothing for an empty list.
pub fn put_group(r: &mut RunReport, prefix: &str, list: &[Recomposite]) {
    if list.is_empty() {
        return;
    }
    r.put_stat(&format!("{prefix}_ms"), Stat::of_durations(&list.iter().map(|x| x.latency).collect::<Vec<_>>()));
    let n = list.len() as f64;
    let mean = |f: &dyn Fn(&SceneStats) -> f64| list.iter().map(|x| f(&x.did)).sum::<f64>() / n;
    r.put(format!("{prefix}.composite_ms"), mean(&|s| ms(s.composite)));
    r.put(format!("{prefix}.prepare_ms"), mean(&|s| ms(s.prepare)));
    r.put(format!("{prefix}.handoff_ms"), mean(&|s| ms(s.handoff)));
    r.put(format!("{prefix}.upload_mb"), mean(&|s| mb(s.upload_bytes)));
    r.put(format!("{prefix}.sets"), mean(&|s| s.sets as f64));
}

/// Puts the whole run's recomposites as `recomposite…` plus their count and upload total.
pub fn put_all(r: &mut RunReport, list: &[Recomposite]) {
    put_group(r, "recomposite", list);
    r.put("recomposites", list.len() as f64);
    r.put("upload_total_mb", list.iter().map(|x| mb(x.did.upload_bytes)).sum());
}
