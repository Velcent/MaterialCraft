//! GPU PBR viewport (OpenGL 3.3 core through glow / egui_glow paint callbacks).
//!
//! Every shape is a tessellated triangle mesh. The height map displaces the vertices in the vertex shader (true
//! displacement), the fragment shader does GGX PBR with normal mapping, outward parallax relief, height-field contact
//! shadows, an analytic studio environment and ACES tone mapping. Textures are mip-mapped with anisotropic filtering
//! and the window is multisampled, so fine texture detail stays sharp at any zoom level.
//!
//! Files: `maps` prepares texture data on the CPU, `textures` / `mesh` hold GL objects, `shaders` the programs,
//! `camera` the matrices, `shape` the built-in preview shape, `scene` custom multi-part models, `pass` the material draw
//! calls and `renderer` ties them into a frame. This file is the public API shared between the UI and the paint callback.
//!
//! # Unsafe exception
//! This module is the documented exception to the workspace's `unsafe_code` lint (see AGENTS.md): glow exposes every
//! OpenGL call as an `unsafe fn` because it can't check that a context is current on the calling thread or that the
//! object handles passed in are alive. The invariant every `unsafe` block here relies on: GL calls are made only from
//! egui_glow paint callbacks (`paint`, `release`) and the functions they call, where egui has made the context
//! current, and every GL object is created and freed by the `Renderer` / `Scene` that owns it, on that same context.
#![allow(unsafe_code)]

mod camera;
mod maps;
mod mesh;
mod pass;
mod renderer;
mod scene;
mod shaders;
mod shape;
mod textures;

pub use maps::{GpuMaps, MapInputs, prepare};
pub use scene::ScenePart;

use crate::render3d::View;
use eframe::egui::PaintCallbackInfo;
use eframe::egui_glow::{self, glow};
use glow::HasContext;
use renderer::Renderer;
use scene::Scene;
use std::sync::{Arc, Mutex};

/// Shared handle between the UI and the paint callback.
pub type Shared = Arc<Mutex<Gpu>>;

pub fn new_shared() -> Shared {
    Arc::new(Mutex::new(Gpu::default()))
}

/// Extra viewer options that live next to (not inside) the CPU renderer's `View`.
#[derive(Clone, Debug)]
pub struct GpuOpts {
    /// Contact shadows computed by marching the height field towards the light.
    pub self_shadow: bool,
    /// Anisotropic filtering level (1..16). 0 = use the maximum the GPU offers.
    pub aniso: f32,
}

impl Default for GpuOpts {
    fn default() -> Self {
        GpuOpts { self_shadow: true, aniso: 0.0 }
    }
}

#[derive(Default)]
pub struct Gpu {
    pub opts: GpuOpts,
    /// Set when shader compilation or a GL call failed; the UI then falls back to the CPU renderer.
    pub error: Option<String>,
    /// Maps waiting to be uploaded on the next paint.
    pending: Option<GpuMaps>,
    /// Request a pixel read-back of the next frame (screenshots, tests).
    pub want_shot: bool,
    /// Result of a read-back: (width, height, RGBA8 rows from the TOP).
    pub shot: Option<(usize, usize, Vec<u8>)>,
    /// GPU name reported by the driver, for the UI.
    pub renderer: String,
    res: Option<Renderer>,
    /// Custom model drawn instead of `View::shape` while active.
    scene: Scene,
    pub frames: u64,
}

impl Gpu {
    pub fn set_maps(&mut self, maps: GpuMaps) {
        self.pending = Some(maps);
    }
    pub fn ready(&self) -> bool {
        self.error.is_none()
    }
    /// Draw these parts instead of `View::shape` until `clear_scene` is called. Uploads happen on the next paint.
    pub fn set_scene(&mut self, parts: Vec<ScenePart>) {
        self.scene.set(parts);
    }
    /// Replace the maps of one part (e.g. after the user edits that texture set). Out-of-range parts are ignored.
    pub fn set_part_maps(&mut self, part: usize, maps: GpuMaps) {
        self.scene.set_part_maps(part, maps);
    }
    /// Go back to the built-in shape; the parts' GPU objects are freed on the next paint.
    pub fn clear_scene(&mut self) {
        self.scene.clear();
    }
    pub fn has_scene(&self) -> bool {
        self.scene.is_active()
    }
}

/// Raw bytes of plain-old-data slices (f32, u32, [u32; 3]) for GL uploads.
// SAFETY: callers pass only plain-old-data element types (f32, u32, [u32; 3]): no padding, every byte initialised;
// the returned slice borrows `v` and covers exactly `size_of_val(v)` bytes of it.
fn bytes_of<T>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}

/// Draw the viewport into the rectangle of the paint callback. Call from an `egui_glow::CallbackFn`.
// SAFETY (every block in `paint`): it runs inside an egui_glow paint callback, so `painter.gl()` is current.
pub fn paint(shared: &Shared, info: &PaintCallbackInfo, painter: &egui_glow::Painter, view: &View) {
    let mut g = match shared.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if g.error.is_some() {
        return;
    }
    let gl: &glow::Context = painter.gl();
    let aniso_req = g.opts.aniso;
    // first use: create everything from the pending maps (or neutral defaults)
    if g.res.is_none() {
        let maps = g
            .pending
            .take()
            .unwrap_or_else(|| prepare(&MapInputs { base: None, normal: None, rough: None, metal: None, ao: None, emissive: None, height: None }));
        match unsafe { Renderer::new(gl, &maps, aniso_req) } {
            Ok(r) => {
                g.renderer = unsafe { gl.get_parameter_string(glow::RENDERER) };
                g.res = Some(r);
            }
            Err(e) => {
                g.error = Some(e);
                return;
            }
        }
    } else if let Some(maps) = g.pending.take() {
        let res = unsafe { g.res.as_mut().unwrap().set_maps(gl, &maps, aniso_req) };
        if let Err(e) = res {
            g.error = Some(e);
            return;
        }
    }
    let g = &mut *g;
    let r = g.res.as_mut().unwrap();
    if let Err(e) = unsafe { g.scene.sync(gl, r.aniso(aniso_req)) } {
        g.error = Some(e);
        return;
    }
    let self_shadow = g.opts.self_shadow;
    let want_shot = std::mem::take(&mut g.want_shot);
    let vp_px = info.viewport_in_pixels();
    if vp_px.width_px <= 0 || vp_px.height_px <= 0 {
        return;
    }
    let aspect = vp_px.width_px as f32 / vp_px.height_px as f32;
    let result = unsafe { r.draw(gl, view, aspect, self_shadow, &g.scene) };
    if want_shot && result.is_ok() {
        g.shot = Some(read_back(gl, info));
    }
    g.frames += 1;
    if let Err(e) = result {
        g.error = Some(e);
    }
}

/// Read the drawn viewport back: (width, height, RGBA8 rows from the top).
// SAFETY: called from `paint` on its current context; `buf` holds exactly w * h RGBA8 pixels of the viewport.
fn read_back(gl: &glow::Context, info: &PaintCallbackInfo) -> (usize, usize, Vec<u8>) {
    let vp_px = info.viewport_in_pixels();
    let (w, h) = (vp_px.width_px as usize, vp_px.height_px as usize);
    let mut buf = vec![0u8; w * h * 4];
    unsafe {
        gl.read_pixels(vp_px.left_px, vp_px.from_bottom_px, w as i32, h as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(&mut buf));
    }
    // flip: GL rows start at the bottom
    let mut top = vec![0u8; w * h * 4];
    for y in 0..h {
        top[y * w * 4..(y + 1) * w * 4].copy_from_slice(&buf[(h - 1 - y) * w * 4..(h - y) * w * 4]);
    }
    (w, h, top)
}

/// Free all GL objects (call when the viewer is closed or the GL context goes away).
// SAFETY: the caller passes the context the objects were created on; `take()` keeps them from being freed twice.
pub fn release(shared: &Shared, gl: &glow::Context) {
    if let Ok(mut g) = shared.lock() {
        if let Some(r) = g.res.take() {
            unsafe { r.free(gl) };
        }
        unsafe { g.scene.free(gl) };
    }
}
