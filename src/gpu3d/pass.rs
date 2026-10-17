//! The material pass: frame-wide uniforms once, then one draw per mesh with that mesh's maps and relief settings.
use super::camera::CamOut;
use super::mesh::GpuMesh;
use super::shaders::Program;
use super::textures::MapTextures;
use crate::render3d::{RELIEF_BUMP, RELIEF_DISPLACE, RELIEF_PARALLAX, View};
use eframe::egui_glow::glow;
use glow::HasContext;

/// One mesh to draw with its own maps.
pub(super) struct Drawable<'a> {
    pub mesh: &'a GpuMesh,
    pub maps: &'a MapTextures,
    /// Mip level of the height map sampled per vertex (see `shape::vtx_lod`).
    pub vtx_lod: f32,
    /// Discard walls facing away from the camera (shapes seen from the inside).
    pub cull: bool,
}

pub(super) struct MaterialPass<'a> {
    gl: &'a glow::Context,
    prog: &'a Program,
    view: &'a View,
    /// Bound as height map for meshes without one.
    flat_height: glow::Texture,
}

// SAFETY: GL calls on the current context (module invariant); the program, meshes and textures it binds are owned
// by the renderer or scene and outlive the pass (its borrows enforce that).
impl<'a> MaterialPass<'a> {
    /// Bind the program and set everything that is the same for all meshes of the frame.
    pub unsafe fn begin(gl: &'a glow::Context, prog: &'a Program, view: &'a View, cam: &CamOut, self_shadow: bool, flat_height: glow::Texture) -> Self {
        unsafe {
            let (ldir, fill) = crate::render3d::light_dirs(view);
            let mode = if view.relief == RELIEF_BUMP {
                1
            } else if view.relief == RELIEF_PARALLAX {
                2
            } else if view.relief == RELIEF_DISPLACE {
                3
            } else {
                0
            };
            gl.use_program(Some(prog.id));
            let loc = |n: &str| prog.loc(n);
            gl.uniform_matrix_4_f32_slice(loc("uVP"), false, &cam.vp);
            gl.uniform_3_f32(loc("uEye"), cam.eye[0], cam.eye[1], cam.eye[2]);
            gl.uniform_3_f32(loc("uL"), ldir[0], ldir[1], ldir[2]);
            gl.uniform_3_f32(loc("uFill"), fill[0], fill[1], fill[2]);
            gl.uniform_1_f32(loc("uLightI"), view.light_intensity);
            gl.uniform_1_f32(loc("uExposure"), view.exposure);
            gl.uniform_1_f32(loc("uAmbient"), view.ambient);
            gl.uniform_1_f32(loc("uNormalAmt"), view.normal_amt.clamp(0.0, 1.0));
            gl.uniform_1_f32(loc("uBump"), view.bump);
            gl.uniform_1_f32(loc("uDepth"), view.height_scale);
            gl.uniform_1_f32(loc("uLevel"), view.height_level);
            gl.uniform_1_i32(loc("uMode"), mode);
            gl.uniform_1_i32(loc("uShadow"), self_shadow as i32);
            gl.uniform_1_i32(loc("uWire"), 0);
            MaterialPass { gl, prog, view, flat_height }
        }
    }

    /// Shade every mesh.
    pub unsafe fn draw(&self, items: &[Drawable]) {
        unsafe {
            for d in items {
                self.bind(d);
                d.mesh.draw(self.gl);
            }
        }
    }

    /// Wireframe overlay of every mesh (displaced like the shaded surface).
    pub unsafe fn draw_wire(&self, items: &[Drawable]) {
        unsafe {
            let gl = self.gl;
            gl.uniform_1_i32(self.prog.loc("uWire"), 1);
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
            gl.enable(glow::POLYGON_OFFSET_LINE);
            gl.polygon_offset(-1.0, -1.0);
            gl.depth_func(glow::LEQUAL);
            for d in items {
                self.bind(d);
                d.mesh.draw(gl);
            }
            gl.disable(glow::POLYGON_OFFSET_LINE);
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.depth_func(glow::LESS);
            gl.uniform_1_i32(self.prog.loc("uWire"), 0);
        }
    }

    /// Per-mesh uniforms and textures.
    unsafe fn bind(&self, d: &Drawable) {
        unsafe {
            let (gl, view, m) = (self.gl, self.view, d.maps);
            let loc = |n: &str| self.prog.loc(n);
            let has_h = m.height.is_some();
            let disp = if has_h && view.relief == RELIEF_DISPLACE { view.height_scale } else { 0.0 };
            let (hw, hh) = m.height_size();
            gl.uniform_1_f32(loc("uDisp"), disp);
            gl.uniform_1_f32(loc("uVtxLod"), d.vtx_lod);
            gl.uniform_1_f32(loc("uTexel"), 1.0 / hw.max(hh).max(1) as f32);
            gl.uniform_1_i32(loc("uHasHeight"), has_h as i32);
            gl.uniform_1_i32(loc("uHasEmis"), m.emis.is_some() as i32);
            gl.uniform_1_i32(loc("uCull"), d.cull as i32);
            let units: [(&str, glow::Texture); 5] = [
                ("uBase", m.base.id),
                ("uNormal", m.normal.id),
                ("uRma", m.rma.id),
                ("uEmis", m.emis.as_ref().map(|t| t.id).unwrap_or(m.base.id)),
                ("uHeight", m.height.as_ref().map(|t| t.id).unwrap_or(self.flat_height)),
            ];
            for (i, (name, tex)) in units.iter().enumerate() {
                gl.active_texture(glow::TEXTURE0 + i as u32);
                gl.bind_texture(glow::TEXTURE_2D, Some(*tex));
                gl.uniform_1_i32(loc(name), i as i32);
            }
        }
    }
}
