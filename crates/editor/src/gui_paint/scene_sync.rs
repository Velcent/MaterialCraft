//! [`SceneSync`]: keeps the GPU scene in step with the open document. Geometry and maps are built
//! on background threads; the scene is set once per geometry build (with neutral placeholder maps)
//! and each texture set's maps replace the placeholders of its parts as they arrive.
//!
//! Edits ask for new maps often (every frame of a slider drag). Only one maps worker runs at a time: a
//! request while one runs cancels it and is started once it has stopped, with every set either asked for.

use super::channel_display::ChannelDisplay;
use super::channel_view::{ChannelView, Shown};
use super::document_state::OpenDocument;
use super::fill_surfaces::FillSurfaces;
use super::geometry::{self, PartGeometry};
use super::job::Job;
use super::maps_worker::{MapsWorker, SetMaps};
use super::quick_mask_tint::QuickMaskTint;
use super::scene_shading::{self, ModelScale, Subdivision};
use super::scene_stats::{self, SceneStats};
use materialcraft::diag::Sink;
use materialcraft::gpu3d::{self, GpuMaps, MapInputs, ScenePart, Shared};
use materialcraft::material_fill::{ChainResolver, FolderResolver, MaterialCache, MaterialFills, MaterialResolver, MemoryResolver};
use materialcraft::paint::edit::LayerRef;
use materialcraft::shading::ShaderLibrary;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

type GeometryResult = Result<Vec<PartGeometry>, String>;

#[derive(Default)]
pub struct SceneSync {
    geometry: Option<Job<GeometryResult>>,
    maps: Option<MapsWorker>,
    /// Texture set of each scene part, once the geometry is on its way to the GPU.
    part_sets: Option<Vec<Option<usize>>>,
    /// Mesh part of each scene part (parts of hidden texture sets are not in the scene).
    mesh_parts: Vec<usize>,
    /// Mesh parts not drawn, by mesh part index (a geometry mask being edited).
    hidden: Vec<bool>,
    /// The texture set the 2D view shows (its parts only).
    uv_set: Option<usize>,
    /// Maps that arrived before the geometry.
    waiting: Vec<SetMaps>,
    /// Maps asked for while a worker was still running: started when it has stopped.
    pending: Option<(ChannelView, BTreeSet<usize>)>,
    /// How channel and mesh-map views show their values (DS-07, DS-08).
    channel: ChannelDisplay,
    /// The layer whose mask is shown alone (MK-05).
    isolate: Option<LayerRef>,
    /// Rendered materials of material fills, kept across maps runs (and documents: keyed by graph content).
    materials: Arc<MaterialCache>,
    /// The asset library roots, where material references are looked up after the document's folder.
    library_folders: Vec<PathBuf>,
    /// The quick mask shown over the view (PT-42).
    tint: Option<Arc<QuickMaskTint>>,
    /// Surface maps of the sets whose fills need them, kept across maps runs (dropped with the document).
    surfaces: Option<Arc<FillSurfaces>>,
    /// The shader files instances are drawn with.
    shaders: ShaderLibrary,
    /// The model's size, measured at the last geometry build.
    model: ModelScale,
    /// Each texture set's subdivision in the last geometry build.
    subdivisions: Vec<Subdivision>,
    pub error: Option<String>,
    /// What keeping the scene in step has cost so far.
    stats: SceneStats,
}

impl SceneSync {
    /// Rebuilds geometry and maps (new document or changed visibility).
    pub fn rebuild(&mut self, doc: &mut OpenDocument, view: ChannelView) {
        let (shared, visible) = (doc.mesh_view(), doc.visibility());
        self.model = ModelScale::of(&shared.mesh);
        self.subdivisions = scene_shading::subdivisions(doc.doc(), &self.model);
        let subdivisions = self.subdivisions.clone();
        self.geometry = Some(Job::spawn(move |_, tx| {
            let _ = tx.send(geometry::build(&shared, &visible, &subdivisions));
        }));
        self.part_sets = None;
        self.error = None;
        self.remap(doc, view);
    }

    /// The shader settings changed: rebuilds the geometry if a set's subdivision changed, else sends the parts'
    /// new shading.
    pub fn shading_changed(&mut self, doc: &mut OpenDocument, view: ChannelView, gpu: &Shared) {
        if self.geometry.is_none() && self.part_sets.is_none() {
            // no scene yet: the next build takes the settings as they are
            return;
        }
        if scene_shading::subdivisions(doc.doc(), &self.model) != self.subdivisions {
            self.rebuild(doc, view);
        } else {
            self.send_shading(doc, gpu);
        }
    }

    /// Sets the shader files instances are drawn with; call [`Self::shading_changed`] after.
    pub fn set_shader_library(&mut self, library: ShaderLibrary) {
        self.shaders = library;
    }

    pub fn shader_library(&self) -> &ShaderLibrary {
        &self.shaders
    }

    /// The model's size as measured at the last geometry build.
    pub fn model_scale(&self) -> &ModelScale {
        &self.model
    }

    /// Gives every scene part its texture set's shading.
    fn send_shading(&self, doc: &OpenDocument, gpu: &Shared) {
        let Some(part_sets) = &self.part_sets else { return };
        let shading = scene_shading::part_shading(doc.doc(), &self.shaders, &self.model, part_sets);
        if let Ok(mut g) = gpu.lock() {
            g.set_part_shading(shading);
        }
    }

    /// Recomputes the maps only (changed display mode).
    pub fn remap(&mut self, doc: &mut OpenDocument, view: ChannelView) {
        doc.invalidate_maps();
        let sets = doc.visible_sets();
        self.refresh(doc, view, sets);
    }

    /// Recomputes the maps of the listed sets (their layers changed); hidden sets are skipped.
    pub fn refresh(&mut self, doc: &mut OpenDocument, view: ChannelView, sets: Vec<usize>) {
        let visible = doc.visible_sets();
        let sets: Vec<usize> = sets.into_iter().filter(|s| visible.contains(s)).collect();
        doc.invalidate_sets(&sets);
        match &self.maps {
            Some(worker) if !worker.is_finished() => {
                worker.cancel();
                let pending = self.pending.get_or_insert_with(|| (view, BTreeSet::new()));
                pending.0 = view;
                pending.1.extend(worker.sets().iter().copied().chain(sets));
            }
            _ => self.start_maps(doc, view, sets),
        }
    }

    fn start_maps(&mut self, doc: &mut OpenDocument, view: ChannelView, sets: Vec<usize>) {
        self.waiting.retain(|m| !sets.contains(&m.set));
        let materials = self.material_fills(doc);
        let surfaces = self.fill_surfaces_for(doc, &sets);
        let shown = Shown { view, channel: self.channel };
        self.maps = Some(MapsWorker::start(doc.maps_view(&sets), sets, shown, self.isolate.clone(), materials, self.tint.clone(), surfaces));
    }

    /// The surface maps the sets' fills composite with, when one of them needs its surface: the kept ones while the
    /// mesh is the same, else new (empty) ones for the current mesh.
    fn fill_surfaces_for(&mut self, doc: &mut OpenDocument, sets: &[usize]) -> Option<Arc<FillSurfaces>> {
        let needed = sets.iter().any(|&i| doc.doc().texture_sets.get(i).is_some_and(|s| s.needs_surface()));
        if !needed {
            return self.surfaces.clone();
        }
        let mesh = doc.shared_mesh();
        if self.surfaces.as_ref().is_none_or(|s| s.revision() != mesh.revision) {
            self.surfaces = Some(Arc::new(FillSurfaces::new(mesh)));
        }
        self.surfaces.clone()
    }

    /// The surface maps of the view's fills (the eyedropper and the layer thumbnails composite with them too).
    pub fn fill_surfaces(&self) -> Option<Arc<FillSurfaces>> {
        self.surfaces.clone()
    }

    /// How material fills of `doc` are evaluated, as the maps the view shows evaluate them (the eyedropper's
    /// stored-value pick composites through the same).
    pub fn material_fills(&self, doc: &OpenDocument) -> MaterialFills {
        MaterialFills::new(Arc::new(material_resolver(doc, &self.library_folders)), self.materials.clone())
    }

    /// Shows `layer`'s mask alone in place of its set's material (`None`: the material again); recomposites.
    pub fn isolate(&mut self, doc: &mut OpenDocument, view: ChannelView, layer: Option<LayerRef>) {
        if self.isolate != layer {
            self.isolate = layer;
            self.remap(doc, view);
        }
    }

    /// Shows channels and mesh maps through `channel`; recomposites when it changed and `view` shows one.
    pub fn set_channel_display(&mut self, doc: &mut OpenDocument, view: ChannelView, channel: ChannelDisplay) {
        if self.channel != channel {
            self.channel = channel;
            if view != ChannelView::Material {
                self.remap(doc, view);
            }
        }
    }

    /// The quick mask tint the next maps runs lay over the sets (`None`: no tint). The caller recomposites.
    pub fn set_tint(&mut self, tint: Option<QuickMaskTint>) {
        self.tint = tint.map(Arc::new);
    }

    /// Drops all work and the GPU scene (rendered materials and the channel display are kept).
    pub fn clear(&mut self, gpu: &Shared) {
        *self = SceneSync {
            materials: self.materials.clone(),
            library_folders: std::mem::take(&mut self.library_folders),
            shaders: std::mem::take(&mut self.shaders),
            channel: self.channel,
            ..SceneSync::default()
        };
        if let Ok(mut g) = gpu.lock() {
            g.clear_scene();
        }
    }

    /// Hands finished work to the GPU and each set's compositing warnings to `sink` (the first
    /// time the set's maps are ready). Call once per frame.
    pub fn poll(&mut self, doc: &mut OpenDocument, gpu: &Shared, sink: &dyn Sink) {
        let mut built = Vec::new();
        if let Some(job) = &mut self.geometry {
            built = job.poll();
            if job.is_finished() {
                self.geometry = None;
            }
        }
        for result in built {
            match result {
                Ok(parts) => {
                    self.set_scene(parts, gpu);
                    self.send_shading(doc, gpu);
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some(worker) = &mut self.maps {
            let arrived = worker.poll();
            // A cancelled run's results are outdated.
            if self.pending.is_none() {
                self.waiting.extend(arrived);
            }
            if worker.is_finished() {
                self.maps = None;
                if let Some((view, sets)) = self.pending.take() {
                    self.start_maps(doc, view, sets.into_iter().collect());
                }
            }
        }
        if let Some(part_sets) = &self.part_sets {
            for SetMaps { set, maps, warnings, timing } in self.waiting.drain(..) {
                let started = std::time::Instant::now();
                self.stats.arrived(timing);
                let bytes = scene_stats::bytes_of(&maps);
                let copies = apply_set_maps(part_sets, set, maps, gpu);
                self.stats.handed(bytes, copies, started.elapsed());
                if doc.mark_ready(set) {
                    warnings.into_iter().for_each(|w| sink.emit(w));
                }
            }
        }
    }

    /// Sets the asset library roots material references are looked up in (used by the next maps run).
    pub fn set_library_folders(&mut self, folders: Vec<PathBuf>) {
        self.library_folders = folders;
    }

    /// True while geometry or maps are still being built.
    pub fn is_busy(&self) -> bool {
        self.geometry.is_some() || self.maps.is_some() || self.pending.is_some() || !self.waiting.is_empty()
    }

    /// What keeping the scene in step has cost so far.
    pub fn stats(&self) -> SceneStats {
        self.stats
    }

    /// (texture sets done, total) of the running maps job.
    pub fn progress(&self) -> Option<(usize, usize)> {
        self.maps.as_ref().map(|m| m.progress())
    }

    /// Hides the mesh parts with `hidden[i]` set in the view (others are drawn); kept for later scenes.
    pub fn set_hidden_parts(&mut self, hidden: Vec<bool>, gpu: &Shared) {
        if self.hidden != hidden {
            self.hidden = hidden;
            self.send_hidden(gpu);
        }
    }

    /// Shows only texture set `set`'s parts in the 2D view (nothing for `None`).
    pub fn show_uv_set(&mut self, set: Option<usize>, gpu: &Shared) {
        if self.uv_set != set {
            self.uv_set = set;
            self.send_hidden(gpu);
        }
    }

    fn send_hidden(&self, gpu: &Shared) {
        let per_scene_part = self.mesh_parts.iter().map(|&p| self.hidden.get(p).copied().unwrap_or(false)).collect();
        let uv_hidden = self.part_sets.as_deref().unwrap_or(&[]).iter().map(|s| s.is_none() || *s != self.uv_set).collect();
        if let Ok(mut g) = gpu.lock() {
            g.set_hidden_parts(per_scene_part);
            g.set_uv_hidden_parts(uv_hidden);
        }
    }

    fn set_scene(&mut self, parts: Vec<PartGeometry>, gpu: &Shared) {
        let part_sets = parts.iter().map(|p| p.set).collect();
        self.mesh_parts = parts.iter().map(|p| p.part).collect();
        let scene = parts.into_iter().map(|p| ScenePart { mesh: p.mesh, tangent_w: p.tangent_w, maps: placeholder() }).collect();
        if let Ok(mut g) = gpu.lock() {
            g.set_scene(scene);
        }
        self.part_sets = Some(part_sets);
        self.send_hidden(gpu);
    }
}

/// Gives every part painted with `set` its maps; returns how many parts got them.
fn apply_set_maps(part_sets: &[Option<usize>], set: usize, maps: GpuMaps, gpu: &Shared) -> usize {
    let parts: Vec<usize> = (0..part_sets.len()).filter(|&i| part_sets[i] == Some(set)).collect();
    let Ok(mut g) = gpu.lock() else { return 0 };
    let mut maps = Some(maps);
    for (k, &part) in parts.iter().enumerate() {
        let m = if k + 1 == parts.len() { maps.take() } else { maps.as_ref().map(copy) };
        if let Some(m) = m {
            g.set_part_maps(part, m);
        }
    }
    parts.len()
}

/// Neutral grey maps shown until a set's own maps are ready.
fn placeholder() -> GpuMaps {
    gpu3d::prepare(&MapInputs { base: None, normal: None, rough: None, metal: None, ao: None, emissive: None, height: None })
}

/// `GpuMaps` is not `Clone`; parts sharing a texture set each need their own copy.
fn copy(m: &GpuMaps) -> GpuMaps {
    GpuMaps { size: m.size, base: m.base.clone(), normal: m.normal.clone(), rma: m.rma.clone(), emissive: m.emissive.clone(), height: m.height.clone() }
}

/// Where material fill references are looked up: the document's embedded assets (AS-21, so it looks the same
/// without the library), then its folder, then the asset library roots.
fn material_resolver(doc: &OpenDocument, library: &[PathBuf]) -> ChainResolver {
    let folders = FolderResolver::new(doc.source().parent().map(|p| p.to_path_buf()).into_iter().chain(library.iter().cloned()));
    ChainResolver::new([Arc::new(MemoryResolver::from_embedded(&doc.doc().assets)) as Arc<dyn MaterialResolver>, Arc::new(folders)])
}
