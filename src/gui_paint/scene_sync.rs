//! [`SceneSync`]: keeps the GPU scene in step with the open document. Geometry and maps are built
//! on background threads; the scene is set once per geometry build (with neutral placeholder maps)
//! and each texture set's maps replace the placeholders of its parts as they arrive.

use super::channel_view::ChannelView;
use super::document_state::OpenDocument;
use super::geometry::{self, PartGeometry};
use super::job::Job;
use super::maps_worker::{MapsWorker, SetMaps};
use materialcraft::gpu3d::{self, GpuMaps, MapInputs, ScenePart, Shared};

type GeometryResult = Result<Vec<PartGeometry>, String>;

#[derive(Default)]
pub struct SceneSync {
    geometry: Option<Job<GeometryResult>>,
    maps: Option<MapsWorker>,
    /// Texture set of each scene part, once the geometry is on its way to the GPU.
    part_sets: Option<Vec<Option<usize>>>,
    /// Maps that arrived before the geometry.
    waiting: Vec<SetMaps>,
    pub error: Option<String>,
}

impl SceneSync {
    /// Rebuilds geometry and maps (new document or changed visibility).
    pub fn rebuild(&mut self, doc: &mut OpenDocument, view: ChannelView) {
        let (shared, visible) = (doc.shared(), doc.visibility());
        self.geometry = Some(Job::spawn(move |_, tx| {
            let _ = tx.send(geometry::build(&shared, &visible));
        }));
        self.part_sets = None;
        self.error = None;
        self.remap(doc, view);
    }

    /// Recomputes the maps only (changed display mode).
    pub fn remap(&mut self, doc: &mut OpenDocument, view: ChannelView) {
        doc.invalidate_maps();
        self.waiting.clear();
        self.maps = Some(MapsWorker::start(doc.shared(), doc.visible_sets(), view));
    }

    /// Drops all work and the GPU scene.
    pub fn clear(&mut self, gpu: &Shared) {
        *self = SceneSync::default();
        if let Ok(mut g) = gpu.lock() {
            g.clear_scene();
        }
    }

    /// Hands finished work to the GPU. Call once per frame.
    pub fn poll(&mut self, doc: &mut OpenDocument, gpu: &Shared) {
        let mut built = Vec::new();
        if let Some(job) = &mut self.geometry {
            built = job.poll();
            if job.is_finished() {
                self.geometry = None;
            }
        }
        for result in built {
            match result {
                Ok(parts) => self.set_scene(parts, gpu),
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(worker) = &mut self.maps {
            self.waiting.extend(worker.poll());
            if worker.is_finished() {
                self.maps = None;
            }
        }
        if let Some(part_sets) = &self.part_sets {
            for SetMaps { set, maps } in self.waiting.drain(..) {
                apply_set_maps(part_sets, set, maps, gpu);
                doc.mark_ready(set);
            }
        }
    }

    /// True while geometry or maps are still being built.
    pub fn is_busy(&self) -> bool {
        self.geometry.is_some() || self.maps.is_some() || !self.waiting.is_empty()
    }

    /// (texture sets done, total) of the running maps job.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.maps.as_ref().map(|m| m.progress())
    }

    fn set_scene(&mut self, parts: Vec<PartGeometry>, gpu: &Shared) {
        let part_sets = parts.iter().map(|p| p.set).collect();
        let scene = parts.into_iter().map(|p| ScenePart { mesh: p.mesh, tangent_w: p.tangent_w, maps: placeholder() }).collect();
        if let Ok(mut g) = gpu.lock() {
            g.set_scene(scene);
        }
        self.part_sets = Some(part_sets);
    }
}

/// Gives every part painted with `set` its maps.
fn apply_set_maps(part_sets: &[Option<usize>], set: usize, maps: GpuMaps, gpu: &Shared) {
    let parts: Vec<usize> = (0..part_sets.len()).filter(|&i| part_sets[i] == Some(set)).collect();
    let Ok(mut g) = gpu.lock() else { return };
    let mut maps = Some(maps);
    for (k, &part) in parts.iter().enumerate() {
        let m = if k + 1 == parts.len() { maps.take() } else { maps.as_ref().map(copy) };
        if let Some(m) = m {
            g.set_part_maps(part, m);
        }
    }
}

/// Neutral grey maps shown until a set's own maps are ready.
fn placeholder() -> GpuMaps {
    gpu3d::prepare(&MapInputs { base: None, normal: None, rough: None, metal: None, ao: None, emissive: None, height: None })
}

/// `GpuMaps` is not `Clone`; parts sharing a texture set each need their own copy.
fn copy(m: &GpuMaps) -> GpuMaps {
    GpuMaps { size: m.size, base: m.base.clone(), normal: m.normal.clone(), rma: m.rma.clone(), emissive: m.emissive.clone(), height: m.height.clone() }
}
