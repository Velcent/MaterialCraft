//! [`MapsWorker`]: composites texture sets into [`MaterialMaps`] and prepares GPU maps for the
//! current display mode ([`Shown`]), off the UI thread. Sets are done one after another so only one set's
//! full-float maps are in memory at a time; each result is sent as soon as it is ready.

use super::channel_view::Shown;
use super::fill_surfaces::FillSurfaces;
use super::job::Job;
use super::quick_mask_tint::QuickMaskTint;
use materialcraft::diag::Entry;
use materialcraft::generators;
use materialcraft::gpu3d::GpuMaps;
use materialcraft::material_fill::MaterialFills;
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::{Document, FillContext, MaterialMaps, OutputOptions, diag_entries};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The prepared maps of one texture set.
pub struct SetMaps {
    pub set: usize,
    pub maps: GpuMaps,
    /// What compositing the set could not do exactly, as log entries.
    pub warnings: Vec<Entry>,
    /// Where the worker's time went (benchmarks and the log).
    pub timing: SetTiming,
}

/// The time a set took on the maps worker.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SetTiming {
    /// Compositing the stack into the set's maps.
    pub composite: Duration,
    /// Turning the maps into GPU maps for the display mode (and the quick mask tint).
    pub prepare: Duration,
}

/// One run over a list of texture sets.
pub struct MapsWorker {
    job: Job<SetMaps>,
    sets: Vec<usize>,
    total: usize,
    done: usize,
}

impl MapsWorker {
    /// Composites `sets` for `shown`, evaluating material fills with `materials`; the set of `isolate` shows that
    /// layer's mask alone instead (MK-05); `tint` lays the quick mask over the maps (PT-42). Sets with polygon fills or
    /// fills projected from a volume composite with their surface map from `surfaces` (built here the first time a set
    /// needs it, then reused by later runs).
    pub fn start(
        doc: Arc<Document>,
        sets: Vec<usize>,
        shown: Shown,
        isolate: Option<LayerRef>,
        materials: MaterialFills,
        tint: Option<Arc<QuickMaskTint>>,
        surfaces: Option<Arc<FillSurfaces>>,
    ) -> MapsWorker {
        let total = sets.len();
        let listed = sets.clone();
        let job = Job::spawn(move |cancel, tx| {
            let fills = FillContext::with_sources(&materials);
            for set in sets {
                if cancel.is_cancelled() {
                    return;
                }
                let ts = &doc.texture_sets[set];
                if let Some(r) = isolate.as_ref().filter(|r| r.set == ts.id)
                    && let Some(maps) = super::isolate::mask_maps(ts, &r.layer, surfaces.as_deref())
                {
                    if tx.send(SetMaps { set, maps, warnings: Vec::new(), timing: SetTiming::default() }).is_err() {
                        return;
                    }
                    continue;
                }
                let started = Instant::now();
                let surface = surfaces.as_ref().filter(|_| ts.needs_surface()).and_then(|s| s.build(ts, ts.resolution));
                if cancel.is_cancelled() {
                    return;
                }
                let fills = surface.as_ref().map_or(fills, |s| s.context(&materials));
                let maps = MaterialMaps::build_with(ts, &OutputOptions::for_document(&doc), generators::builtin(), &fills);
                if cancel.is_cancelled() {
                    return;
                }
                let composited = Instant::now();
                let warnings = diag_entries::composite_warnings(ts, &maps.warnings);
                let mut maps = shown.prepare(&maps, ts);
                if let Some(t) = &tint {
                    t.apply(&ts.id, &mut maps);
                }
                let timing = SetTiming { composite: composited - started, prepare: composited.elapsed() };
                if tx.send(SetMaps { set, maps, warnings, timing }).is_err() {
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

#[cfg(test)]
#[path = "maps_worker_tests.rs"]
mod tests;
