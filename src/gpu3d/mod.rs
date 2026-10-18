//! GPU PBR viewport (OpenGL 3.3 core through glow / egui_glow paint callbacks).
//!
//! Every shape is a tessellated triangle mesh. The height map displaces the vertices in the vertex shader (true
//! displacement), the fragment shader does GGX PBR with normal mapping, outward parallax relief, height-field contact
//! shadows and an analytic studio environment, writing linear radiance into a half-float target multisampled like the
//! window. A post stage then applies exposure in stops, the tone curve (filmic by default, linear or an ACES fit) and
//! the display transform, and blends the result over the background. While the view is still, jittered frames
//! (and, with depth of field, frames through points of the lens) accumulate into a running average; any change
//! starts again from the plain frame, so moving costs nothing extra. Unlit shading shows the maps as they are and
//! skips exposure and the curve. Textures are mip-mapped with anisotropic filtering and an optional sharpening bias.
//!
//! Files: `maps` prepares texture data on the CPU, `textures` / `mesh` hold GL objects, `shaders` the programs,
//! `camera` the matrices, `shape` the built-in preview shape, `scene` custom multi-part models, `pass` the material draw
//! calls, `target` the HDR and accumulation framebuffers, `progress` which accumulation sample is next, `display` the
//! display settings and `renderer` ties them into a frame. This file is the public API shared between the UI and the
//! paint callback.
//!
//! # Unsafe exception
//! This module is the documented exception to the workspace's `unsafe_code` lint (see AGENTS.md): glow exposes every
//! OpenGL call as an `unsafe fn` because it can't check that a context is current on the calling thread or that the
//! object handles passed in are alive. The invariant every `unsafe` block here relies on: GL calls are made only from
//! egui_glow paint callbacks (`paint`, `release`) and the functions they call, where egui has made the context
//! current, and every GL object is created and freed by the `Renderer` / `Scene` that owns it, on that same context.
#![allow(unsafe_code)]

mod camera;
mod display;
mod maps;
mod mesh;
mod pass;
mod progress;
mod renderer;
mod scene;
mod shaders;
mod shape;
mod target;
mod textures;

pub use camera::view_camera;
pub use display::{Antialiasing, Display, MIP_BIAS_STEPS, Shading, ToneCurve};
pub use maps::{GpuMaps, MapInputs, prepare};
pub use scene::ScenePart;

use crate::diag::{Area, Sink, SinkExt};
use crate::render3d::View;
use eframe::egui::PaintCallbackInfo;
use eframe::egui_glow::{self, glow};
use glow::HasContext;
use renderer::{Frame, Renderer, ViewRect};
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

/// The depth buffer of one drawn frame and the camera it was drawn with (on a viewport in pixels).
pub struct DepthShot {
    pub width: usize,
    pub height: usize,
    /// Window depth (0 near … 1 far; 1 where nothing was drawn), rows from the top.
    pub depth: Vec<f32>,
    pub camera: crate::camera::Camera,
}

/// The displayed colour at one point of a drawn frame (the viewport eyedropper).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorShot {
    /// Where it was read, in UI points from the viewport's top-left corner (as requested).
    pub at: [f32; 2],
    /// The pixel as drawn, display-encoded (sRGB) RGBA8; `None` if the point is outside the view or the driver
    /// refused the read.
    pub rgba: Option<[u8; 4]>,
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
    /// Request a depth read-back of the next frame (checks of the CPU view projector against the GPU).
    pub want_depth: bool,
    pub depth: Option<DepthShot>,
    /// Request the displayed colour at this point (UI points from the viewport's top-left corner) of the next frame.
    pub want_color: Option<[f32; 2]>,
    /// The answer to `want_color`, set by the next drawn frame.
    pub color: Option<ColorShot>,
    /// GPU name reported by the driver, for the UI.
    pub renderer: String,
    res: Option<Renderer>,
    /// Custom model drawn instead of `View::shape` while active.
    scene: Scene,
    pub frames: u64,
    /// Bumped by every map or scene change, so the accumulation restarts when the content changes.
    content: u64,
    /// The last frame left the accumulation incomplete: paint again soon (the view is still refining).
    refining: bool,
    /// The anisotropic filtering request the uploaded textures have.
    applied_aniso: Option<f32>,
}

impl Gpu {
    pub fn set_maps(&mut self, maps: GpuMaps) {
        self.pending = Some(maps);
        self.content += 1;
    }

    /// True while the last drawn frame wants more accumulation samples: the UI should paint again.
    pub fn refining(&self) -> bool {
        self.refining && self.error.is_none()
    }
    pub fn ready(&self) -> bool {
        self.error.is_none()
    }
    /// Draw these parts instead of `View::shape` until `clear_scene` is called. Uploads happen on the next paint.
    pub fn set_scene(&mut self, parts: Vec<ScenePart>) {
        self.scene.set(parts);
        self.content += 1;
    }
    /// Replace the maps of one part (e.g. after the user edits that texture set). Out-of-range parts are ignored.
    pub fn set_part_maps(&mut self, part: usize, maps: GpuMaps) {
        self.scene.set_part_maps(part, maps);
        self.content += 1;
    }
    /// Hide parts of the scene (`hidden[i]` for part `i`, as passed to `set_scene`); parts beyond the list are drawn.
    /// Kept when the scene is replaced, so the caller sends it again for a new scene.
    pub fn set_hidden_parts(&mut self, hidden: Vec<bool>) {
        self.scene.set_hidden(hidden);
    }
    /// Go back to the built-in shape; the parts' GPU objects are freed on the next paint.
    pub fn clear_scene(&mut self) {
        self.scene.clear();
        self.content += 1;
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

/// Draw the viewport into the rectangle of the paint callback with [`Display::plain`] (each change drawn once, no
/// accumulation). Call from an `egui_glow::CallbackFn`.
pub fn paint(shared: &Shared, info: &PaintCallbackInfo, painter: &egui_glow::Painter, view: &View, sink: &dyn Sink) {
    paint_with(shared, info, painter, view, &Display::plain(), sink);
}

/// Draw the viewport into the rectangle of the paint callback with `display`'s camera, post stage and accumulation.
/// While [`Gpu::refining`] is true the UI should keep painting. Shader compile and link failures and GL errors go to
/// `sink` once, when they stop the viewer; the GPU's name is reported when the renderer starts.
// SAFETY (every block in `paint_with`): it runs inside an egui_glow paint callback, so `painter.gl()` is current.
pub fn paint_with(shared: &Shared, info: &PaintCallbackInfo, painter: &egui_glow::Painter, view: &View, display: &Display, sink: &dyn Sink) {
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
                sink.info(Area::Gpu, format!("3D view started on {}.", g.renderer));
                g.res = Some(r);
            }
            Err(e) => {
                fail(&mut g.error, sink, e);
                return;
            }
        }
    } else if let Some(maps) = g.pending.take() {
        let res = unsafe { g.res.as_mut().unwrap().set_maps(gl, &maps, aniso_req) };
        if let Err(e) = res {
            fail(&mut g.error, sink, e);
            return;
        }
    }
    let g = &mut *g;
    let r = g.res.as_mut().unwrap();
    if let Err(e) = unsafe { g.scene.sync(gl, r.aniso(aniso_req)) } {
        fail(&mut g.error, sink, e);
        return;
    }
    // a changed filtering request applies to the textures already uploaded
    // (only where the GPU offers anisotropic filtering: the parameter is unknown otherwise)
    if g.applied_aniso.is_some_and(|a| a != aniso_req) && r.aniso(f32::MAX) > 1.0 {
        let level = r.aniso(aniso_req);
        unsafe {
            r.set_aniso(gl, level);
            g.scene.set_aniso(gl, level);
        }
    }
    g.applied_aniso = Some(aniso_req);
    let self_shadow = g.opts.self_shadow;
    let want_shot = std::mem::take(&mut g.want_shot);
    let want_depth = std::mem::take(&mut g.want_depth);
    let want_color = g.want_color.take();
    let vp_px = info.viewport_in_pixels();
    if vp_px.width_px <= 0 || vp_px.height_px <= 0 {
        return;
    }
    let frame = Frame {
        view,
        display,
        self_shadow,
        aniso_req,
        content: g.content,
        rect: ViewRect { left: vp_px.left_px, bottom: vp_px.from_bottom_px, width: vp_px.width_px, height: vp_px.height_px },
        restart: want_depth,
    };
    let result = unsafe { r.draw(gl, &frame, &g.scene) };
    g.refining = r.refining(display);
    if want_shot && result.is_ok() {
        g.shot = Some(read_back(gl, info));
    }
    if want_depth && let Ok(camera) = &result {
        let (width, height) = (vp_px.width_px as usize, vp_px.height_px as usize);
        g.depth = r.target().and_then(|t| read_depth(gl, t)).map(|depth| DepthShot { width, height, depth, camera: *camera });
    }
    if let Some(at) = want_color
        && result.is_ok()
    {
        g.color = Some(ColorShot { at, rgba: read_color(gl, info, at) });
    }
    g.frames += 1;
    if let Err(e) = result {
        fail(&mut g.error, sink, e);
    }
}

/// Stops the viewer with `error` and reports it.
fn fail(slot: &mut Option<String>, sink: &dyn Sink, error: String) {
    sink.error(Area::Gpu, format!("The 3D view stopped: {error}"));
    *slot = Some(error);
}

/// Read the drawn viewport back: (width, height, RGBA8 rows from the top).
// SAFETY: called from `paint` on its current context; `buf` holds exactly w * h RGBA8 pixels of the viewport.
fn read_back(gl: &glow::Context, info: &PaintCallbackInfo) -> (usize, usize, Vec<u8>) {
    let vp_px = info.viewport_in_pixels();
    let (w, h) = (vp_px.width_px as usize, vp_px.height_px as usize);
    let mut buf = vec![0u8; w * h * 4];
    unsafe {
        gl.read_pixels(vp_px.left_px, vp_px.from_bottom_px, w as i32, h as i32, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut buf)));
    }
    // flip: GL rows start at the bottom
    let mut top = vec![0u8; w * h * 4];
    for y in 0..h {
        top[y * w * 4..(y + 1) * w * 4].copy_from_slice(&buf[(h - 1 - y) * w * 4..(h - y) * w * 4]);
    }
    (w, h, top)
}

/// Read the scene's resolved depth back (rows from the top), or `None` if the driver refuses (the frame stays
/// drawn; a depth read-back is a check, not part of drawing).
// SAFETY: called from `paint_with` on its current context; `buf` holds exactly w * h f32 depth values of the target.
fn read_depth(gl: &glow::Context, target: &target::HdrTarget) -> Option<Vec<f32>> {
    let (w, h) = (target.width.max(0) as usize, target.height.max(0) as usize);
    let mut buf = vec![0f32; w * h];
    let ok = unsafe {
        // clear errors left by earlier calls (bounded: a lost context keeps reporting one)
        for _ in 0..16 {
            if gl.get_error() == glow::NO_ERROR {
                break;
            }
        }
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(target.resolved_fbo()));
        gl.read_pixels(0, 0, w as i32, h as i32, glow::DEPTH_COMPONENT, glow::FLOAT, glow::PixelPackData::Slice(Some(bytes_of_mut(&mut buf))));
        let ok = gl.get_error() == glow::NO_ERROR;
        gl.bind_framebuffer(glow::READ_FRAMEBUFFER, None);
        ok
    };
    if !ok {
        return None;
    }
    Some((0..h).rev().flat_map(|y| buf[y * w..(y + 1) * w].iter().copied()).collect())
}

/// Read the drawn colour at `at` (UI points from the viewport's top-left corner), or `None` outside the viewport or if
/// the driver refuses (the frame stays drawn).
// SAFETY: called from `paint` on its current context; `buf` holds exactly one RGBA8 pixel, read from inside the viewport.
fn read_color(gl: &glow::Context, info: &PaintCallbackInfo, at: [f32; 2]) -> Option<[u8; 4]> {
    let vp_px = info.viewport_in_pixels();
    let ppp = info.pixels_per_point;
    let (x, y) = ((at[0] * ppp).floor(), (at[1] * ppp).floor());
    if !(x >= 0.0 && y >= 0.0 && x < vp_px.width_px as f32 && y < vp_px.height_px as f32) {
        return None;
    }
    // GL rows start at the bottom
    let (gx, gy) = (vp_px.left_px + x as i32, vp_px.from_bottom_px + vp_px.height_px - 1 - y as i32);
    let mut buf = [0u8; 4];
    let ok = unsafe {
        for _ in 0..16 {
            if gl.get_error() == glow::NO_ERROR {
                break;
            }
        }
        gl.read_pixels(gx, gy, 1, 1, glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelPackData::Slice(Some(&mut buf)));
        gl.get_error() == glow::NO_ERROR
    };
    ok.then_some(buf)
}

/// Raw writable bytes of an f32 slice for GL read-backs.
// SAFETY: f32 has no padding and every bit pattern is a valid f32; the returned slice borrows `v` mutably and covers
// exactly `size_of_val(v)` bytes of it.
fn bytes_of_mut(v: &mut [f32]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(v.as_mut_ptr() as *mut u8, std::mem::size_of_val(v)) }
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
