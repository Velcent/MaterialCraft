//! [`MapsWorker`]: composites texture sets into [`MaterialMaps`] and prepares GPU maps for the
//! current display mode ([`Shown`]), off the UI thread. Sets are done one after another so only one set's
//! full-float maps are in memory at a time; each result is sent as soon as it is ready.
//!
//! A set can be asked for whole or for some of its tiles ([`Want`]): a brush stroke asks for the tiles it painted,
//! and gets a patch for the maps on the GPU ([`super::maps_patch`]) when the set allows it, else the whole set.

use super::channel_view::Shown;
use super::fill_surfaces::FillSurfaces;
use super::job::Job;
use super::maps_patch;
use super::quick_mask_tint::QuickMaskTint;
use materialcraft::diag::Entry;
use materialcraft::generators;
use materialcraft::gpu3d::{GpuMaps, MapsPatch};
use materialcraft::material_fill::MaterialFills;
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::stack::StackCache;
use materialcraft::paint::{Document, FillContext, MaterialMaps, OutputOptions, diag_entries};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What a run should make of one set.
#[derive(Debug, Clone, PartialEq)]
pub enum Want {
    /// The whole set.
    Full,
    /// The tiles whose composite changed (tile coordinates of the set's grid); the maps on the GPU are otherwise
    /// up to date for the run's display mode.
    Tiles(Vec<(u32, u32)>),
}

impl Want {
    /// Both wants at once: the whole set if either is, else the tiles of both.
    pub fn merged(self, other: Want) -> Want {
        match (self, other) {
            (Want::Tiles(mut a), Want::Tiles(b)) => {
                a.extend(b);
                a.sort_unstable();
                a.dedup();
                Want::Tiles(a)
            }
            _ => Want::Full,
        }
    }
}

/// A set's new maps on the GPU.
pub enum SetContent {
    /// The whole maps.
    Full(GpuMaps),
    /// Some tiles of the maps already there.
    Patch(MapsPatch),
}

/// The prepared maps of one texture set.
pub struct SetMaps {
    pub set: usize,
    pub content: SetContent,
    /// What compositing the set could not do exactly, as log entries (none for a patch).
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

/// What every set of a run is composited with.
pub struct RunSetup {
    pub shown: Shown,
    /// The layer whose mask is shown alone (MK-05).
    pub isolate: Option<LayerRef>,
    pub materials: MaterialFills,
    /// The quick mask laid over the maps (PT-42).
    pub tint: Option<Arc<QuickMaskTint>>,
    pub surfaces: Option<Arc<FillSurfaces>>,
    /// Expensive masks kept across runs (the same maps as without).
    pub cache: Option<Arc<StackCache>>,
}

/// One run over a list of texture sets.
pub struct MapsWorker {
    job: Job<SetMaps>,
    jobs: Vec<(usize, Want)>,
    done: usize,
}

impl MapsWorker {
    /// Composites `jobs` for `setup.shown`, evaluating material fills with `setup.materials`; the set of
    /// `setup.isolate` shows that layer's mask alone instead (MK-05); `setup.tint` lays the quick mask over the maps
    /// (PT-42). Sets with polygon fills or fills projected from a volume composite with their surface map from
    /// `setup.surfaces` (built here the first time a set needs it, then reused by later runs).
    pub fn start(doc: Arc<Document>, jobs: Vec<(usize, Want)>, setup: RunSetup) -> MapsWorker {
        let listed = jobs.clone();
        let job = Job::spawn(move |cancel, tx| {
            let RunSetup { shown, isolate, materials, tint, surfaces, cache } = setup;
            let plain = FillContext::with_sources(&materials);
            let options = OutputOptions::for_document(&doc);
            for (set, want) in jobs {
                if cancel.is_cancelled() {
                    return;
                }
                let Some(ts) = doc.texture_sets.get(set) else { continue };
                let isolated = isolate.as_ref().filter(|r| r.set == ts.id);
                if let Some(r) = isolated
                    && let Some(maps) = super::isolate::mask_maps(ts, &r.layer, surfaces.as_deref())
                {
                    if tx.send(SetMaps { set, content: SetContent::Full(maps), warnings: Vec::new(), timing: SetTiming::default() }).is_err() {
                        return;
                    }
                    continue;
                }
                let started = Instant::now();
                let surface = surfaces.as_ref().filter(|_| ts.needs_surface()).and_then(|s| s.build(ts, ts.resolution));
                if cancel.is_cancelled() {
                    return;
                }
                let fills = surface.as_ref().map_or(plain, |s| s.context(&materials));
                if let Want::Tiles(tiles) = &want
                    && tint.is_none()
                    && isolated.is_none()
                    && let Some(patch) = maps_patch::patch(ts, &options, &fills, &shown, tiles)
                {
                    let timing = SetTiming { composite: started.elapsed(), prepare: Duration::ZERO };
                    if tx.send(SetMaps { set, content: SetContent::Patch(patch), warnings: Vec::new(), timing }).is_err() {
                        return;
                    }
                    continue;
                }
                // what the fill context gives masks: the surface maps of this mesh, or none
                let context = surface.as_ref().map_or(0, |_| surfaces.as_ref().map_or(0, |s| s.revision().wrapping_add(1)));
                let session = cache.as_ref().map(|c| c.session(ts, context));
                let maps = MaterialMaps::build_cached(ts, &options, generators::builtin(), &fills, session.as_ref());
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
                if tx.send(SetMaps { set, content: SetContent::Full(maps), warnings, timing }).is_err() {
                    return;
                }
            }
        });
        MapsWorker { job, jobs: listed, done: 0 }
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

    /// What this run was started for.
    pub fn jobs(&self) -> &[(usize, Want)] {
        &self.jobs
    }

    pub fn is_finished(&self) -> bool {
        self.job.is_finished()
    }

    /// (sets done, sets total)
    pub fn progress(&self) -> (usize, usize) {
        (self.done, self.jobs.len())
    }
}

#[cfg(test)]
#[path = "maps_worker_tests.rs"]
mod tests;
