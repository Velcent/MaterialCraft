//! The built-in preview shape (`View::shape`), rebuilt on the GPU when shape, tessellation or tiling change.
use super::mesh::GpuMesh;
use crate::render3d::{View, build_mesh};
use eframe::egui_glow::glow;

#[derive(Default)]
pub(super) struct ShapeMesh {
    mesh: Option<GpuMesh>,
    key: (u32, u32, u32),
}

// SAFETY: GL calls on the current context (module invariant); a replaced mesh is freed right after the swap and
// never used again.
impl ShapeMesh {
    /// The mesh for the view's shape, rebuilt first when it changed.
    pub unsafe fn update(&mut self, gl: &glow::Context, v: &View) -> Result<&GpuMesh, String> {
        unsafe {
            let key = (v.shape, v.tess.clamp(8, 768), (v.tiling.max(0.1) * 1000.0) as u32);
            if self.mesh.is_none() || self.key != key {
                let m = build_mesh(v.shape, key.1, v.tiling, None, 0.0, 0.0);
                let new = GpuMesh::upload(gl, &m, &[])?;
                if let Some(old) = self.mesh.replace(new) {
                    old.free(gl);
                }
                self.key = key;
            }
            Ok(self.mesh.as_ref().unwrap())
        }
    }

    pub unsafe fn free(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(m) = self.mesh.take() {
                m.free(gl);
            }
        }
    }
}

/// Mip level the vertex shader samples the height map at: about one texel per vertex of the shape's grid.
pub(super) fn vtx_lod(view: &View, height_w: usize) -> f32 {
    let mesh_n = {
        let n = view.tess.clamp(8, 768);
        match view.shape {
            1 | 4 | 5 | 9 => (n * 5 / 8).max(8),
            6 => (n * 2).min(1024),
            _ => n,
        }
    } as f32;
    ((height_w as f32 * view.tiling.max(0.1) / mesh_n).max(1.0)).log2()
}
