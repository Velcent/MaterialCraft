//! Custom scenes: several meshes, each with its own maps (a model's parts / texture sets), drawn instead of the
//! built-in shape. Changes are queued on the UI side and applied on the GL thread at the next paint.
use super::GpuMaps;
use super::mesh::{GpuMesh, uv_density};
use super::pass::Drawable;
use super::shading::PartShading;
use super::textures::MapTextures;
use crate::render3d::Mesh;
use eframe::egui_glow::glow;

/// One part of a custom model: its mesh (already scaled to about unit size by the caller) and its maps.
pub struct ScenePart {
    pub mesh: Mesh,
    /// Bitangent sign per vertex (glTF / MikkTSpace tangent w: bitangent = cross(N, T) * w, pointing along -v; -1 on
    /// mirrored uv islands). Empty = all +1.
    pub tangent_w: Vec<f32>,
    pub maps: GpuMaps,
}

/// Changes waiting for the next paint.
#[derive(Default)]
enum Pending {
    #[default]
    Nothing,
    /// New parts (empty when the scene was cleared).
    Replace(Vec<ScenePart>),
    /// New maps for existing parts.
    Maps(Vec<(usize, GpuMaps)>),
}

/// A part as uploaded to the GPU.
struct GpuPart {
    mesh: GpuMesh,
    maps: MapTextures,
    /// Vertices per uv unit (for the vertex height-map LOD).
    density: f32,
}

// SAFETY (GpuPart and Scene): GL calls on the current context (module invariant); parts are uploaded and freed
// only by the scene that owns them.
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
            Ok(GpuPart { mesh, maps, density: uv_density(&p.mesh) })
        }
    }

    unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            self.mesh.free(gl);
            self.maps.free(gl);
        }
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
}

impl Scene {
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set(&mut self, parts: Vec<ScenePart>) {
        self.active = true;
        self.pending = Pending::Replace(parts);
    }

    pub fn clear(&mut self) {
        self.active = false;
        self.pending = Pending::Replace(Vec::new());
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

    pub fn set_part_maps(&mut self, part: usize, maps: GpuMaps) {
        match &mut self.pending {
            Pending::Replace(parts) => {
                if let Some(p) = parts.get_mut(part) {
                    p.maps = maps;
                }
            }
            Pending::Maps(list) => {
                list.retain(|(i, _)| *i != part);
                list.push((part, maps));
            }
            Pending::Nothing => self.pending = Pending::Maps(vec![(part, maps)]),
        }
    }

    /// Apply queued changes: upload new parts / maps and free what they replace.
    pub unsafe fn sync(&mut self, gl: &glow::Context, aniso: f32) -> Result<(), String> {
        unsafe {
            match std::mem::take(&mut self.pending) {
                Pending::Nothing => {}
                Pending::Replace(parts) => {
                    self.free(gl);
                    self.parts.reserve(parts.len());
                    for p in &parts {
                        self.parts.push(GpuPart::upload(gl, p, aniso)?);
                    }
                }
                Pending::Maps(list) => {
                    for (i, maps) in list {
                        if let Some(p) = self.parts.get_mut(i) {
                            let new = MapTextures::upload(gl, &maps, aniso)?;
                            std::mem::replace(&mut p.maps, new).free(gl);
                        }
                    }
                }
            }
            Ok(())
        }
    }

    /// Every part that is not hidden, with its own maps; no inside-view culling (open meshes show their back faces).
    pub fn drawables(&self) -> Vec<Drawable<'_>> {
        self.parts
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.hidden.get(*i).copied().unwrap_or(false))
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
