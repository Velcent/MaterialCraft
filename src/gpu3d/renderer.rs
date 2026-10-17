//! GL resources that live as long as the viewport (programs, the built-in shape and its maps) and the frame they draw.
use super::camera::camera;
use super::pass::{Drawable, MaterialPass};
use super::scene::Scene;
use super::shaders::Program;
use super::shape::{self, ShapeMesh};
use super::textures::{aniso_level, max_aniso, MapTextures, Tx};
use super::GpuMaps;
use crate::render3d::View;
use eframe::egui_glow::glow;
use glow::HasContext;

pub(super) struct Renderer {
    prog: Program,
    bg_prog: Program,
    /// Empty vertex array for the attribute-less background triangle.
    bg_vao: glow::VertexArray,
    shape: ShapeMesh,
    maps: MapTextures,
    flat_height: Tx,
    max_aniso: f32,
}

// SAFETY: GL calls on the current context (module invariant); the renderer owns every object it creates and
// releases them in `free`, which consumes it.
impl Renderer {
    pub unsafe fn new(gl: &glow::Context, maps: &GpuMaps, aniso_req: f32) -> Result<Renderer, String> { unsafe {
        let prog = Program::material(gl)?;
        let bg_prog = Program::background(gl)?;
        let max_aniso = max_aniso(gl);
        let bg_vao = gl.create_vertex_array()?;
        let maps = MapTextures::upload(gl, maps, aniso_level(aniso_req, max_aniso))?;
        let flat_height = Tx::flat_height(gl)?;
        Ok(Renderer { prog, bg_prog, bg_vao, shape: ShapeMesh::default(), maps, flat_height, max_aniso })
    }}

    pub fn aniso(&self, req: f32) -> f32 {
        aniso_level(req, self.max_aniso)
    }

    /// Replace the maps of the built-in shape.
    pub unsafe fn set_maps(&mut self, gl: &glow::Context, maps: &GpuMaps, aniso_req: f32) -> Result<(), String> { unsafe {
        let new = MapTextures::upload(gl, maps, self.aniso(aniso_req))?;
        std::mem::replace(&mut self.maps, new).free(gl);
        Ok(())
    }}

    /// Draw background and the active scene (or else the built-in shape) into the current viewport.
    pub unsafe fn draw(&mut self, gl: &glow::Context, view: &View, aspect: f32, self_shadow: bool, scene: &Scene) -> Result<(), String> { unsafe {
        let cam = camera(view, aspect);
        let items = if scene.is_active() {
            scene.drawables()
        } else {
            let mesh = self.shape.update(gl, view)?;
            vec![Drawable { mesh, maps: &self.maps, vtx_lod: shape::vtx_lod(view, self.maps.height_size().0), cull: crate::render3d::shape_culls(view.shape) }]
        };
        gl.enable(glow::DEPTH_TEST);
        gl.depth_func(glow::LESS);
        gl.depth_mask(true);
        gl.clear_depth_f32(1.0);
        gl.clear(glow::DEPTH_BUFFER_BIT);
        // background gradient (no depth write)
        gl.depth_mask(false);
        gl.use_program(Some(self.bg_prog.id));
        gl.bind_vertex_array(Some(self.bg_vao));
        gl.draw_arrays(glow::TRIANGLES, 0, 3);
        gl.depth_mask(true);
        // material pass
        let pass = MaterialPass::begin(gl, &self.prog, view, &cam, self_shadow, self.flat_height.id);
        pass.draw(&items);
        if view.wireframe {
            pass.draw_wire(&items);
        }
        // leave the state the way egui expects it
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, None);
        gl.bind_vertex_array(None);
        gl.disable(glow::DEPTH_TEST);
        let e = gl.get_error();
        if e != glow::NO_ERROR {
            return Err(format!("OpenGL error 0x{e:x} while drawing the 3D view"));
        }
        Ok(())
    }}

    pub unsafe fn free(mut self, gl: &glow::Context) { unsafe {
        self.prog.free(gl);
        self.bg_prog.free(gl);
        gl.delete_vertex_array(self.bg_vao);
        self.shape.free(gl);
        self.maps.free(gl);
        self.flat_height.free(gl);
    }}
}
