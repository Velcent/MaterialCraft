//! [`MapsWorker`]: composites texture sets into [`MaterialMaps`] and prepares GPU maps for the
//! current [`ChannelView`], off the UI thread. Sets are done one after another so only one set's
//! full-float maps are in memory at a time; each result is sent as soon as it is ready.

use super::channel_view::ChannelView;
use super::job::Job;
use super::quick_mask_tint::QuickMaskTint;
use materialcraft::diag::Entry;
use materialcraft::generators;
use materialcraft::gpu3d::GpuMaps;
use materialcraft::material_fill::MaterialFills;
use materialcraft::mesh::{SurfaceKey, SurfaceMap};
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::{Document, FillContext, MaterialMaps, OutputOptions, TextureSet, diag_entries};
use std::sync::Arc;

/// The set's surface map (no gutter) for polygon fills; `None` when the view has no mesh for it.
fn surface_map(doc: &Document, ts: &TextureSet) -> Option<SurfaceMap> {
    let (width, height) = ts.resolution;
    let key = SurfaceKey { material: ts.material_slot, uv_set: ts.uv_set, width, height, gutter: 0, mesh_revision: 0 };
    SurfaceMap::build(&doc.mesh, key).ok()
}

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
    /// Composites `sets` for `view`, evaluating material fills with `materials`; the set of `isolate` shows that
    /// layer's mask alone instead (MK-05); `tint` lays the quick mask over the maps (PT-42).
    pub fn start(
        doc: Arc<Document>,
        sets: Vec<usize>,
        view: ChannelView,
        isolate: Option<LayerRef>,
        materials: MaterialFills,
        tint: Option<Arc<QuickMaskTint>>,
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
                    && let Some(maps) = super::isolate::mask_maps(ts, &r.layer)
                {
                    if tx.send(SetMaps { set, maps, warnings: Vec::new() }).is_err() {
                        return;
                    }
                    continue;
                }
                let surface = ts.uses_polygon_fill().then(|| surface_map(&doc, ts)).flatten();
                let fills = match &surface {
                    Some(s) => fills.with_surface(s, None),
                    None => fills,
                };
                let maps = MaterialMaps::build_with(ts, &OutputOptions::for_document(&doc), generators::builtin(), &fills);
                if cancel.is_cancelled() {
                    return;
                }
                let warnings = diag_entries::composite_warnings(ts, &maps.warnings);
                let mut maps = view.prepare(&maps, ts);
                if let Some(t) = &tint {
                    t.apply(&ts.id, &mut maps);
                }
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
