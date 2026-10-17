//! [`MapsWorker`]: composites texture sets into [`MaterialMaps`] and prepares GPU maps for the
//! current [`ChannelView`], off the UI thread. Sets are done one after another so only one set's
//! full-float maps are in memory at a time; each result is sent as soon as it is ready.

use super::channel_view::ChannelView;
use super::job::Job;
use materialcraft::diag::Entry;
use materialcraft::gpu3d::GpuMaps;
use materialcraft::paint::{Document, MaterialMaps, OutputOptions, diag_entries};
use std::sync::Arc;

/// The prepared maps of one texture set.
pub struct SetMaps {
    pub set: usize,
    pub maps: GpuMaps,
    /// What compositing the set could not do exactly, as log entries.
    pub warnings: Vec<Entry>,
}

/// One run over a list of texture sets.
pub struct MapsWorker {
    job: Job<SetMaps>,
    sets: Vec<usize>,
    total: usize,
    done: usize,
}

impl MapsWorker {
    pub fn start(doc: Arc<Document>, sets: Vec<usize>, view: ChannelView) -> MapsWorker {
        let total = sets.len();
        let listed = sets.clone();
        let job = Job::spawn(move |cancel, tx| {
            for set in sets {
                if cancel.is_cancelled() {
                    return;
                }
                let ts = &doc.texture_sets[set];
                let maps = MaterialMaps::build(ts, &OutputOptions::for_document(&doc));
                if cancel.is_cancelled() {
                    return;
                }
                let warnings = diag_entries::composite_warnings(ts, &maps.warnings);
                let maps = view.prepare(&maps, ts);
                if tx.send(SetMaps { set, maps, warnings }).is_err() {
                    return;
                }
            }
        });
        MapsWorker { job, sets: listed, total, done: 0 }
    }

    /// Sets finished since the last call.
    pub fn poll(&mut self) -> Vec<SetMaps> {
        let out = self.job.poll();
        self.done += out.len();
        out
    }

    /// Stops after the set being composited now (its result and later ones are not sent).
    pub fn cancel(&self) {
        self.job.cancel();
    }

    /// The sets this run was started for.
    pub fn sets(&self) -> &[usize] {
        &self.sets
    }

    pub fn is_finished(&self) -> bool {
        self.job.is_finished()
    }

    /// (sets done, sets total)
    pub fn progress(&self) -> (usize, usize) {
        (self.done, self.total)
    }
}
