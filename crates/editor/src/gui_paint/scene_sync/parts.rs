//! The GPU scene's parts: their shading, which of them are hidden, and setting a new scene from built geometry.

use super::SceneSync;
use crate::gui_paint::channel_view::ChannelView;
use crate::gui_paint::document_state::OpenDocument;
use crate::gui_paint::geometry::PartGeometry;
use crate::gui_paint::scene_shading::{self, ModelScale};
use materialcraft::gpu3d::{self, GpuMaps, MapInputs, ScenePart, Shared};
use materialcraft::shading::ShaderLibrary;

impl SceneSync {
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
    pub(super) fn send_shading(&self, doc: &OpenDocument, gpu: &Shared) {
        let Some(part_sets) = &self.part_sets else { return };
        let shading = scene_shading::part_shading(doc.doc(), &self.shaders, &self.model, part_sets);
        if let Ok(mut g) = gpu.lock() {
            g.set_part_shading(shading);
        }
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

    pub(super) fn set_scene(&mut self, parts: Vec<PartGeometry>, gpu: &Shared) {
        let part_sets = parts.iter().map(|p| p.set).collect();
        self.mesh_parts = parts.iter().map(|p| p.part).collect();
        let scene = parts.into_iter().map(|p| ScenePart { mesh: p.mesh, tangent_w: p.tangent_w, maps: placeholder() }).collect();
        if let Ok(mut g) = gpu.lock() {
            g.set_scene(scene);
        }
        self.part_sets = Some(part_sets);
        // the new parts show placeholders until their whole maps arrive
        self.uploaded.clear();
        self.send_hidden(gpu);
    }
}

/// Neutral grey maps shown until a set's own maps are ready.
fn placeholder() -> GpuMaps {
    gpu3d::prepare(&MapInputs { base: None, normal: None, rough: None, metal: None, ao: None, emissive: None, height: None })
}
