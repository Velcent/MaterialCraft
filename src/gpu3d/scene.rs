//! Custom scenes: several meshes, each with its own maps (a model's parts / texture sets), drawn instead of the
//! built-in shape. Changes are queued on the UI side and applied on the GL thread at the next paint.
use super::mesh::{uv_density, GpuMesh};
use super::pass::Drawable;
use super::textures::MapTextures;
use super::GpuMaps;
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

impl GpuPart {
    unsafe fn upload(gl: &glow::Context, p: &ScenePart, aniso: f32) -> Result<GpuPart, String> {
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

    unsafe fn free(&self, gl: &glow::Context) {
        self.mesh.free(gl);
        self.maps.free(gl);
    }
}

#[derive(Default)]
pub(super) struct Scene {
    active: bool,
    pending: Pending,
    parts: Vec<GpuPart>,
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

    /// Every part with its own maps; no inside-view culling (open meshes show their back faces).
    pub fn drawables(&self) -> Vec<Drawable<'_>> {
        self.parts
            .iter()
            .map(|p| {
                let hw = p.maps.height_size().0;
                Drawable { mesh: &p.mesh, maps: &p.maps, vtx_lod: (hw as f32 / p.density).max(1.0).log2(), cull: false }
            })
            .collect()
    }

    /// Free the GPU objects of all parts (queued changes stay queued).
    pub unsafe fn free(&mut self, gl: &glow::Context) {
        for p in self.parts.drain(..) {
            p.free(gl);
        }
    }
}
