//! Custom scenes: several meshes, each with its own maps (a model's parts / texture sets), drawn instead of the
//! built-in shape. Changes are queued on the UI side and applied on the GL thread at the next paint.
//!
//! Parts painted with the same maps (the mesh parts of one texture set) share one set of textures: new maps for
//! several parts are uploaded once. A patch replaces a rectangle of maps already uploaded (a brush stroke's tiles)
//! instead of uploading them whole.
use super::GpuMaps;
use super::maps::MapsPatch;
use super::mesh::{GpuMesh, uv_density};
use super::pass::Drawable;
use super::shading::PartShading;
use super::textures::MapTextures;
use crate::render3d::Mesh;
use eframe::egui_glow::glow;
use std::sync::Arc;

/// One part of a custom model: its mesh (already scaled to about unit size by the caller) and its maps.
pub struct ScenePart {
    pub mesh: Mesh,
    /// Bitangent sign per vertex (glTF / MikkTSpace tangent w: bitangent = cross(N, T) * w, pointing along -v; -1 on
    /// mirrored uv islands). Empty = all +1.
    pub tangent_w: Vec<f32>,
    pub maps: GpuMaps,
}

/// A change of the maps of existing parts, applied in the order queued.
enum Change {
    /// New maps shared by the parts.
    Maps { parts: Vec<usize>, maps: GpuMaps },
    /// A rectangle of the parts' maps.
    Patch { parts: Vec<usize>, patch: MapsPatch },
}

impl Change {
    fn parts(&self) -> &[usize] {
        match self {
            Change::Maps { parts, .. } | Change::Patch { parts, .. } => parts,
        }
    }
}

/// Changes waiting for the next paint.
#[derive(Default)]
struct Pending {
    /// New parts (empty when the scene was cleared), uploaded before `changes`.
    replace: Option<Vec<ScenePart>>,
    changes: Vec<Change>,
}

/// A part as uploaded to the GPU.
struct GpuPart {
    mesh: GpuMesh,
    /// Shared with the other parts painted with the same maps.
    maps: Arc<MapTextures>,
    /// Vertices per uv unit (for the vertex height-map LOD).
    density: f32,
}

// SAFETY (GpuPart and Scene): GL calls on the current context (module invariant); parts are uploaded and freed
// only by the scene that owns them, and shared maps are freed by whoever drops the last reference to them.
impl GpuPart {
    unsafe fn upload(gl: &glow::Context, p: &ScenePart, aniso: f32) -> Result<GpuPart, String> {
        unsafe {
            let mesh = GpuMesh::upload(gl, &p.mesh, &p.tangent_w)?;
            let maps = match MapTextures::upload(gl, &p.maps, aniso) {
                Ok(m) => m,
                Err(e) => {
                    mesh.free(gl);
                    return Err(e);
                }
            };
            Ok(GpuPart { mesh, maps: Arc::new(maps), density: uv_density(&p.mesh) })
        }
    }

    unsafe fn free(self, gl: &glow::Context) {
        unsafe {
            self.mesh.free(gl);
            release(gl, self.maps);
        }
    }
}

/// Drops one reference to shared maps, freeing them with the last.
unsafe fn release(gl: &glow::Context, maps: Arc<MapTextures>) {
    if let Some(m) = Arc::into_inner(maps) {
        unsafe { m.free(gl) };
    }
}

#[derive(Default)]
pub(super) struct Scene {
    active: bool,
    pending: Pending,
    parts: Vec<GpuPart>,
    /// Parts not drawn, by index (a geometry mask being edited hides what it excludes). Kept across uploads.
    hidden: Vec<bool>,
    /// Each part's shading by index; parts beyond the list use the built-in default. Kept across uploads.
    shading: Vec<PartShading>,
    default_shading: PartShading,
    /// Parts the 2D view leaves out besides `hidden` (it shows one texture set's parts). Kept across uploads.
    uv_hidden: Vec<bool>,
}

impl Scene {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set(&mut self, parts: Vec<ScenePart>) {
        self.active = true;
        self.pending = Pending { replace: Some(parts), changes: Vec::new() };
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.pending = Pending { replace: Some(Vec::new()), changes: Vec::new() };
    }

    /// Which parts are not drawn (`hidden[i]` for part `i`; parts beyond the list are drawn). Takes effect at the
    /// next paint, without uploading anything.
    pub fn set_hidden(&mut self, hidden: Vec<bool>) {
        self.hidden = hidden;
    }

    /// Each part's shading (`shading[i]` for part `i`; parts beyond the list use the built-in default). Takes
    /// effect at the next paint; programs of new shader files are compiled then.
    pub fn set_shading(&mut self, shading: Vec<PartShading>) {
        self.shading = shading;
    }

    /// The shader files the parts use (each once).
    pub fn shader_files(&self) -> Vec<&crate::shading::ShaderFile> {
        let mut out: Vec<&crate::shading::ShaderFile> = Vec::new();
        for s in self.shading.iter().take(self.parts.len()) {
            if !out.iter().any(|f| f.program_key == s.shader.program_key) {
                out.push(&s.shader);
            }
        }
        out
    }

    /// Which parts the 2D view leaves out besides the hidden ones (`hidden[i]` for part `i`).
    pub fn set_uv_hidden(&mut self, hidden: Vec<bool>) {
        self.uv_hidden = hidden;
    }

    /// New maps shared by `parts`; replaces what is queued for no other parts than these.
    pub fn set_parts_maps(&mut self, parts: Vec<usize>, maps: GpuMaps) {
        self.pending.changes.retain(|c| !c.parts().iter().all(|p| parts.contains(p)));
        self.pending.changes.push(Change::Maps { parts, maps });
    }

    /// New texels for a rectangle of the maps of `parts`.
    pub fn patch_parts_maps(&mut self, parts: Vec<usize>, patch: MapsPatch) {
        self.pending.changes.push(Change::Patch { parts, patch });
    }

    /// Apply queued changes: upload new parts / maps and free what they replace.
    pub unsafe fn sync(&mut self, gl: &glow::Context, aniso: f32) -> Result<(), String> {
        unsafe {
            let Pending { replace, changes } = std::mem::take(&mut self.pending);
            if let Some(parts) = replace {
                self.free(gl);
                self.parts.reserve(parts.len());
                for p in &parts {
                    self.parts.push(GpuPart::upload(gl, p, aniso)?);
                }
            }
            for change in changes {
                match change {
                    Change::Maps { parts, maps } => {
                        if !parts.iter().any(|&i| i < self.parts.len()) {
                            continue;
                        }
                        let new = Arc::new(MapTextures::upload(gl, &maps, aniso)?);
                        for i in parts {
                            if let Some(p) = self.parts.get_mut(i) {
                                release(gl, std::mem::replace(&mut p.maps, new.clone()));
                            }
                        }
                    }
                    Change::Patch { parts, patch } => {
                        let mut done: Vec<&Arc<MapTextures>> = Vec::new();
                        for p in parts.iter().filter_map(|&i| self.parts.get(i)) {
                            if !done.iter().any(|d| Arc::ptr_eq(d, &p.maps)) {
                                p.maps.patch(gl, &patch);
                                done.push(&p.maps);
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// Every part that is not hidden (in the 2D view if `uv`), with its own maps; no inside-view culling (open
    /// meshes show their back faces).
    pub fn drawables(&self, uv: bool) -> Vec<Drawable<'_>> {
        let off = |list: &[bool], i: usize| list.get(i).copied().unwrap_or(false);
        self.parts
            .iter()
            .enumerate()
            .filter(|(i, _)| !off(&self.hidden, *i) && (!uv || !off(&self.uv_hidden, *i)))
            .map(|(i, p)| {
                let hw = p.maps.height_size().0;
                let shading = self.shading.get(i).unwrap_or(&self.default_shading);
                Drawable { mesh: &p.mesh, maps: &p.maps, vtx_lod: (hw as f32 / p.density).max(1.0).log2(), cull: false, shading }
            })
            .collect()
    }

    /// Changes the anisotropic filtering level of every part's maps.
    pub unsafe fn set_aniso(&self, gl: &glow::Context, aniso: f32) {
        unsafe {
            for p in &self.parts {
                p.maps.set_aniso(gl, aniso);
            }
        }
    }

    /// Free the GPU objects of all parts (queued changes stay queued).
    pub unsafe fn free(&mut self, gl: &glow::Context) {
        unsafe {
            for p in self.parts.drain(..) {
                p.free(gl);
            }
        }
    }
}
