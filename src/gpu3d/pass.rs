//! The material pass: frame-wide uniforms once, then one draw per mesh with that mesh's maps and relief settings.
use super::camera::CamOut;
use super::mesh::GpuMesh;
use super::shaders::Program;
use super::shading::{PartShading, send_uniforms};
use super::textures::MapTextures;
use crate::render3d::{RELIEF_BUMP, RELIEF_DISPLACE, RELIEF_PARALLAX, View};
use eframe::egui_glow::glow;
use glow::HasContext;

/// The texture unit of the scene depth the wireframe tests against (after the five map units).
const WIRE_DEPTH_UNIT: u32 = 5;

/// One mesh to draw with its own maps.
pub(super) struct Drawable<'a> {
    pub mesh: &'a GpuMesh,
    pub maps: &'a MapTextures,
    /// Mip level of the height map sampled per vertex (see `shape::vtx_lod`).
    pub vtx_lod: f32,
    /// Discard walls facing away from the camera (shapes seen from the inside).
    pub cull: bool,
    /// The shader instance's program, values and displacement.
    pub shading: &'a PartShading,
}

/// Frame-wide shading switches.
pub(super) struct Shade {
    /// Contact shadows from the height field.
    pub self_shadow: bool,
    /// Show the maps as they are, without light (the post stage then leaves them alone).
    pub unlit: bool,
    /// Mip-map bias (0 … −3: sharper texture sampling).
    pub mip_bias: f32,
}

/// What the wireframe overlay tests its lines against: the scene's resolved depth.
pub(super) struct WireDepth {
    pub texture: glow::Texture,
    /// The viewport's lower-left corner in window pixels (where the depth texture's texel 0, 0 lies).
    pub origin: [f32; 2],
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
    pub unsafe fn begin(gl: &'a glow::Context, prog: &'a Program, view: &'a View, cam: &CamOut, shade: &Shade, flat_height: glow::Texture) -> Self {
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
            gl.uniform_1_i32(loc("uShadow"), shade.self_shadow as i32);
            gl.uniform_1_i32(loc("uUnlit"), shade.unlit as i32);
            gl.uniform_1_f32(loc("uMipBias"), shade.mip_bias);
            gl.uniform_1_i32(loc("uWire"), 0);
            let d = cam.depth_lin;
            gl.uniform_4_f32(loc("uDepthLin"), d[0], d[1], d[2], d[3]);
            MaterialPass { gl, prog, view, flat_height }
        }
    }

    /// Shade every mesh (all drawn with this pass's program).
    pub unsafe fn draw<'d, 'm: 'd>(&self, items: impl IntoIterator<Item = &'d Drawable<'m>>) {
        unsafe {
            for d in items {
                self.bind(d);
                d.mesh.draw(self.gl);
            }
        }
    }

    /// Wireframe overlay of every mesh (displaced like the shaded surface), drawn into the current framebuffer after
    /// the post stage: lines behind the scene's surface (by `depth`) are left out by the shader.
    pub unsafe fn draw_wire(&self, items: &[Drawable], depth: &WireDepth) {
        unsafe {
            let gl = self.gl;
            let loc = |n: &str| self.prog.loc(n);
            gl.uniform_1_i32(loc("uWire"), 1);
            gl.uniform_2_f32(loc("uDepthOrigin"), depth.origin[0], depth.origin[1]);
            gl.active_texture(glow::TEXTURE0 + WIRE_DEPTH_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, Some(depth.texture));
            gl.uniform_1_i32(loc("uSceneDepth"), WIRE_DEPTH_UNIT as i32);
            gl.disable(glow::DEPTH_TEST);
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
            for d in items {
                self.bind(d);
                d.mesh.draw(gl);
            }
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.uniform_1_i32(loc("uWire"), 0);
            gl.active_texture(glow::TEXTURE0 + WIRE_DEPTH_UNIT);
            gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    /// Per-mesh uniforms and textures.
    unsafe fn bind(&self, d: &Drawable) {
        unsafe {
            let (gl, view, m) = (self.gl, self.view, d.maps);
            let loc = |n: &str| self.prog.loc(n);
            let has_h = m.height.is_some();
            // the instance's displacement, else the view's height display
            let (disp, level) = match d.shading.displacement {
                Some(x) => (if has_h { x.distance } else { 0.0 }, x.level),
                None => (if has_h && view.relief == RELIEF_DISPLACE { view.height_scale } else { 0.0 }, view.height_level),
            };
            let (hw, hh) = m.height_size();
            gl.uniform_1_f32(loc("uDisp"), disp);
            gl.uniform_1_f32(loc("uDispLevel"), level);
            send_uniforms(gl, self.prog, &d.shading.uniforms);
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
