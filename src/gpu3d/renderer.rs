//! GL resources that live as long as the viewport (programs, the built-in shape and its maps, the HDR targets) and
//! the frame they draw: the scene into the HDR target (one accumulation sample), the background and the post stage
//! into egui's framebuffer, then the wireframe overlay.
use super::GpuMaps;
use super::camera::CamOut;
use super::display::{Display, Shading};
use super::pass::{Drawable, MaterialPass, Shade, WireDepth};
use super::progress::{FrameKey, Progress, Step};
use super::scene::Scene;
use super::shaders::Program;
use super::shape::{self, ShapeMesh};
use super::target::{Accumulation, HdrTarget};
use super::textures::{MapTextures, Tx, aniso_level, max_aniso};
use crate::camera::{Camera, lens_point, pixel_jitter};
use crate::render3d::View;
use eframe::egui_glow::glow;
use glow::HasContext;

/// Where in egui's framebuffer the view goes, in window pixels (GL's lower-left origin).
#[derive(Debug, Clone, Copy)]
pub(super) struct ViewRect {
    pub left: i32,
    pub bottom: i32,
    pub width: i32,
    pub height: i32,
}

/// One paint's input besides the scene.
pub(super) struct Frame<'a> {
    pub view: &'a View,
    pub display: &'a Display,
    pub self_shadow: bool,
    pub aniso_req: f32,
    /// The scene's content generation (bumped by every upload request).
    pub content: u64,
    pub rect: ViewRect,
    /// Draw the plain (unjittered) frame now, for read-backs.
    pub restart: bool,
}

pub(super) struct Renderer {
    prog: Program,
    bg_prog: Program,
    post_prog: Program,
    accum_prog: Program,
    /// Empty vertex array for the attribute-less full-screen triangles.
    bg_vao: glow::VertexArray,
    shape: ShapeMesh,
    maps: MapTextures,
    flat_height: Tx,
    max_aniso: f32,
    /// Samples per pixel of egui's framebuffer, used for the HDR target too.
    samples: i32,
    target: Option<HdrTarget>,
    accum: Option<Accumulation>,
    progress: Progress,
}

// SAFETY: GL calls on the current context (module invariant); the renderer owns every object it creates and
// releases them in `free`, which consumes it.
impl Renderer {
    pub unsafe fn new(gl: &glow::Context, maps: &GpuMaps, aniso_req: f32) -> Result<Renderer, String> {
        unsafe {
            let prog = Program::material(gl)?;
            let bg_prog = Program::background(gl)?;
            let post_prog = Program::post(gl)?;
            let accum_prog = Program::accumulate(gl)?;
            let max_aniso = max_aniso(gl);
            let bg_vao = gl.create_vertex_array()?;
            let maps = MapTextures::upload(gl, maps, aniso_level(aniso_req, max_aniso))?;
            let flat_height = Tx::flat_height(gl)?;
            // the framebuffer bound for the paint callback is egui's: match its multisampling
            let samples = gl.get_parameter_i32(glow::SAMPLES).clamp(0, gl.get_parameter_i32(glow::MAX_SAMPLES).max(0));
            Ok(Renderer {
                prog,
                bg_prog,
                post_prog,
                accum_prog,
                bg_vao,
                shape: ShapeMesh::default(),
                maps,
                flat_height,
                max_aniso,
                samples,
                target: None,
                accum: None,
                progress: Progress::default(),
            })
        }
    }

    pub fn aniso(&self, req: f32) -> f32 {
        aniso_level(req, self.max_aniso)
    }

    /// True while a still view wants more accumulation samples.
    pub fn refining(&self, display: &Display) -> bool {
        self.progress.refining(display.frames())
    }

    /// Replace the maps of the built-in shape.
    pub unsafe fn set_maps(&mut self, gl: &glow::Context, maps: &GpuMaps, aniso_req: f32) -> Result<(), String> {
        unsafe {
            let new = MapTextures::upload(gl, maps, self.aniso(aniso_req))?;
            std::mem::replace(&mut self.maps, new).free(gl);
            Ok(())
        }
    }

    /// Changes the anisotropic filtering level of the built-in shape's maps (only where the GPU offers it).
    pub unsafe fn set_aniso(&self, gl: &glow::Context, aniso: f32) {
        unsafe { self.maps.set_aniso(gl, aniso) }
    }

    /// The resolved HDR target (depth read-backs).
    pub fn target(&self) -> Option<&HdrTarget> {
        self.target.as_ref()
    }

    /// Draws the frame: a scene sample into the HDR target when one is due, then background, post stage and
    /// wireframe into egui's framebuffer at `frame.rect`. Returns the plain camera of the frame.
    pub unsafe fn draw(&mut self, gl: &glow::Context, frame: &Frame, scene: &Scene) -> Result<Camera, String> {
        unsafe {
            let ViewRect { left, bottom, width: w, height: h } = frame.rect;
            let (view, display) = (frame.view, frame.display);
            let camera = view.camera(&display.camera, [w as f32, h as f32]);
            self.ensure_targets(gl, w, h)?;
            let key = FrameKey::new(view, display, (frame.self_shadow, frame.aniso_req), frame.content, (w, h));
            let frames = display.frames();
            let step = self.progress.next(key, frames, frame.restart);
            // egui's scissor box: restored for the passes into its framebuffer
            let mut scissor = [0i32; 4];
            gl.get_parameter_i32_slice(glow::SCISSOR_BOX, &mut scissor);
            if !scene.is_active() {
                self.shape.update(gl, view)?;
            }
            let items = if scene.is_active() { scene.drawables() } else { self.shape_drawable(view) };
            if let Step::Draw { index } = step {
                gl.disable(glow::SCISSOR_TEST);
                gl.disable(glow::BLEND);
                if index == 1 {
                    // the plain frame still in the resolved target starts the average
                    self.accumulate(gl, 1.0);
                }
                let sample = sample_camera(&camera, display, index, frames, [w as f32, h as f32]);
                self.draw_scene(gl, view, display, frame.self_shadow, &sample, &items);
                if index >= 1 {
                    self.accumulate(gl, 1.0 / (index + 1) as f32);
                }
            }
            // into egui's framebuffer
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.viewport(left, bottom, w, h);
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(scissor[0], scissor[1], scissor[2], scissor[3]);
            gl.disable(glow::DEPTH_TEST);
            gl.depth_mask(false);
            gl.use_program(Some(self.bg_prog.id));
            gl.bind_vertex_array(Some(self.bg_vao));
            gl.disable(glow::BLEND);
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            self.post(gl, display, [left as f32, bottom as f32]);
            gl.depth_mask(true);
            if view.wireframe
                && let Some(depth) = self.target.as_ref().and_then(HdrTarget::depth)
            {
                let shade = Shade { self_shadow: false, unlit: false, mip_bias: 0.0 };
                let pass = MaterialPass::begin(gl, &self.prog, view, &CamOut::of(&camera), &shade, self.flat_height.id);
                pass.draw_wire(&items, &WireDepth { texture: depth, origin: [left as f32, bottom as f32] });
            }
            // leave the state the way egui expects it
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_vertex_array(None);
            gl.disable(glow::DEPTH_TEST);
            gl.enable(glow::BLEND);
            let e = gl.get_error();
            if e != glow::NO_ERROR {
                return Err(format!("OpenGL error 0x{e:x} while drawing the 3D view"));
            }
            Ok(camera)
        }
    }

    /// The built-in shape (updated for `view` first) as the one thing to draw.
    fn shape_drawable(&self, view: &View) -> Vec<Drawable<'_>> {
        let cull = crate::render3d::shape_culls(view.shape);
        let vtx_lod = shape::vtx_lod(view, self.maps.height_size().0);
        self.shape.current().map(|mesh| Drawable { mesh, maps: &self.maps, vtx_lod, cull }).into_iter().collect()
    }

    /// (Re)creates the HDR and accumulation targets for a view of `w` × `h` pixels.
    unsafe fn ensure_targets(&mut self, gl: &glow::Context, w: i32, h: i32) -> Result<(), String> {
        unsafe {
            if self.target.as_ref().is_some_and(|t| t.fits(w, h, self.samples)) {
                return Ok(());
            }
            self.free_targets(gl);
            self.progress.invalidate();
            self.target = Some(HdrTarget::new(gl, w, h, self.samples)?);
            self.accum = Some(Accumulation::new(gl, w, h)?);
            Ok(())
        }
    }

    /// One scene sample into the HDR target, resolved.
    unsafe fn draw_scene(&self, gl: &glow::Context, view: &View, display: &Display, self_shadow: bool, camera: &Camera, items: &[Drawable]) {
        unsafe {
            let Some(target) = &self.target else { return };
            target.bind_draw(gl);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.depth_mask(true);
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            let shade = Shade { self_shadow, unlit: display.shading == Shading::Unlit, mip_bias: display.mip_bias() };
            let pass = MaterialPass::begin(gl, &self.prog, view, &CamOut::of(camera), &shade, self.flat_height.id);
            pass.draw(items);
            gl.disable(glow::DEPTH_TEST);
            target.resolve(gl);
        }
    }

    /// Weighs the resolved frame into the running average: average = frame · `weight` + average · (1 − `weight`).
    unsafe fn accumulate(&self, gl: &glow::Context, weight: f32) {
        unsafe {
            let (Some(target), Some(accum)) = (&self.target, &self.accum) else { return };
            accum.bind_draw(gl);
            gl.enable(glow::BLEND);
            gl.blend_equation(glow::FUNC_ADD);
            gl.blend_color(0.0, 0.0, 0.0, weight);
            gl.blend_func(glow::CONSTANT_ALPHA, glow::ONE_MINUS_CONSTANT_ALPHA);
            gl.use_program(Some(self.accum_prog.id));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(target.color()));
            gl.uniform_1_i32(self.accum_prog.loc("uSrc"), 0);
            gl.bind_vertex_array(Some(self.bg_vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.disable(glow::BLEND);
        }
    }

    /// Exposure, tone curve and display transform of the picture, blended over the background.
    unsafe fn post(&self, gl: &glow::Context, display: &Display, origin: [f32; 2]) {
        unsafe {
            let source = match (&self.accum, &self.target) {
                (Some(a), _) if self.progress.averaged() => a.color(),
                (_, Some(t)) => t.color(),
                _ => return,
            };
            let p = &self.post_prog;
            gl.use_program(Some(p.id));
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(source));
            gl.uniform_1_i32(p.loc("uSrc"), 0);
            gl.uniform_2_f32(p.loc("uOrigin"), origin[0], origin[1]);
            gl.uniform_1_f32(p.loc("uExposure"), display.exposure_scale());
            gl.uniform_1_i32(p.loc("uCurve"), display.tone_curve.shader_index());
            gl.uniform_1_i32(p.loc("uBypass"), (display.shading == Shading::Unlit) as i32);
            // premultiplied colour over the background (egui's own blend)
            gl.enable(glow::BLEND);
            gl.blend_equation(glow::FUNC_ADD);
            gl.blend_func_separate(glow::ONE, glow::ONE_MINUS_SRC_ALPHA, glow::ONE_MINUS_DST_ALPHA, glow::ONE);
            gl.bind_vertex_array(Some(self.bg_vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.disable(glow::BLEND);
        }
    }

    unsafe fn free_targets(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(t) = self.target.take() {
                t.free(gl);
            }
            if let Some(a) = self.accum.take() {
                a.free(gl);
            }
        }
    }

    pub unsafe fn free(mut self, gl: &glow::Context) {
        unsafe {
            self.free_targets(gl);
            self.prog.free(gl);
            self.bg_prog.free(gl);
            self.post_prog.free(gl);
            self.accum_prog.free(gl);
            gl.delete_vertex_array(self.bg_vao);
            self.shape.free(gl);
            self.maps.free(gl);
            self.flat_height.free(gl);
        }
    }
}

/// The camera of accumulation sample `index` of `frames`: sub-pixel jitter when anti-aliasing accumulates, a point
/// of the lens when depth of field is on; sample 0 is the plain camera.
fn sample_camera(camera: &Camera, display: &Display, index: u32, frames: u32, size: [f32; 2]) -> Camera {
    let mut c = *camera;
    if index == 0 {
        return c;
    }
    if display.antialiasing.frames() > 1 {
        c = c.jittered(pixel_jitter(index), size);
    }
    if display.camera.blurs() {
        let dof = &display.camera.depth_of_field;
        let p = lens_point(index, frames);
        c = c.through_lens([p[0] * dof.aperture, p[1] * dof.aperture], dof.focus_distance);
    }
    c
}
