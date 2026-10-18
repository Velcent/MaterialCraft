//! [`HdrTarget`]: the off-screen frame the scene is drawn into: half-float colour (linear radiance, coverage in
//! alpha) and depth, multisampled like the window and resolved into textures the post stage, the accumulation
//! and the wireframe overlay read. [`Accumulation`]: the running average of jittered frames (PP-02). [`ViewTargets`]:
//! both for one view (the 3D view and the 2D view each keep their own, so both can be shown side by side).

use super::progress::Progress;
use eframe::egui_glow::glow;
use glow::HasContext;

/// A colour (and optionally depth) texture with its framebuffer.
struct Attached {
    fbo: glow::Framebuffer,
    color: glow::Texture,
    depth: Option<glow::Texture>,
}

/// A multisampled colour and depth framebuffer of renderbuffers.
struct Multisampled {
    fbo: glow::Framebuffer,
    color: glow::Renderbuffer,
    depth: glow::Renderbuffer,
}

pub(super) struct HdrTarget {
    pub width: i32,
    pub height: i32,
    samples: i32,
    ms: Option<Multisampled>,
    resolved: Attached,
}

// SAFETY (whole file): GL calls on the current context (module invariant); every object is created and freed by
// the target that owns it.
unsafe fn texture(gl: &glow::Context, internal: u32, format: u32, ty: u32, w: i32, h: i32) -> Result<glow::Texture, String> {
    unsafe {
        let t = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(t));
        gl.tex_image_2d(glow::TEXTURE_2D, 0, internal as i32, w, h, 0, format, ty, glow::PixelUnpackData::Slice(None));
        for (k, v) in [(glow::TEXTURE_MIN_FILTER, glow::NEAREST), (glow::TEXTURE_MAG_FILTER, glow::NEAREST)] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, k, v as i32);
        }
        for k in [glow::TEXTURE_WRAP_S, glow::TEXTURE_WRAP_T] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, k, glow::CLAMP_TO_EDGE as i32);
        }
        gl.bind_texture(glow::TEXTURE_2D, None);
        Ok(t)
    }
}

unsafe fn complete(gl: &glow::Context, what: &str) -> Result<(), String> {
    unsafe {
        let status = gl.check_framebuffer_status(glow::FRAMEBUFFER);
        gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        if status == glow::FRAMEBUFFER_COMPLETE { Ok(()) } else { Err(format!("the {what} framebuffer is incomplete (0x{status:x})")) }
    }
}

impl Attached {
    unsafe fn new(gl: &glow::Context, internal: u32, w: i32, h: i32, with_depth: bool) -> Result<Attached, String> {
        unsafe {
            let fbo = gl.create_framebuffer()?;
            let color = texture(gl, internal, glow::RGBA, glow::FLOAT, w, h)?;
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(color), 0);
            let depth = if with_depth {
                let d = texture(gl, glow::DEPTH_COMPONENT24, glow::DEPTH_COMPONENT, glow::UNSIGNED_INT, w, h)?;
                gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::TEXTURE_2D, Some(d), 0);
                Some(d)
            } else {
                None
            };
            let a = Attached { fbo, color, depth };
            if let Err(e) = complete(gl, "viewport") {
                a.free(gl);
                return Err(e);
            }
            Ok(a)
        }
    }

    unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.fbo);
            gl.delete_texture(self.color);
            if let Some(d) = self.depth {
                gl.delete_texture(d);
            }
        }
    }
}

impl Multisampled {
    unsafe fn new(gl: &glow::Context, samples: i32, w: i32, h: i32) -> Result<Multisampled, String> {
        unsafe {
            let fbo = gl.create_framebuffer()?;
            let color = gl.create_renderbuffer()?;
            let depth = gl.create_renderbuffer()?;
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(color));
            gl.renderbuffer_storage_multisample(glow::RENDERBUFFER, samples, glow::RGBA16F, w, h);
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(depth));
            gl.renderbuffer_storage_multisample(glow::RENDERBUFFER, samples, glow::DEPTH_COMPONENT24, w, h);
            gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
            gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::RENDERBUFFER, Some(color));
            gl.framebuffer_renderbuffer(glow::FRAMEBUFFER, glow::DEPTH_ATTACHMENT, glow::RENDERBUFFER, Some(depth));
            let m = Multisampled { fbo, color, depth };
            if let Err(e) = complete(gl, "multisampled viewport") {
                m.free(gl);
                return Err(e);
            }
            Ok(m)
        }
    }

    unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_framebuffer(self.fbo);
            gl.delete_renderbuffer(self.color);
            gl.delete_renderbuffer(self.depth);
        }
    }
}

impl HdrTarget {
    /// A target of `w` × `h` pixels with `samples` samples per pixel (0 or 1: no multisampling).
    pub unsafe fn new(gl: &glow::Context, w: i32, h: i32, samples: i32) -> Result<HdrTarget, String> {
        unsafe {
            let resolved = Attached::new(gl, glow::RGBA16F, w, h, true)?;
            let ms = if samples > 1 {
                match Multisampled::new(gl, samples, w, h) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        resolved.free(gl);
                        return Err(e);
                    }
                }
            } else {
                None
            };
            Ok(HdrTarget { width: w, height: h, samples, ms, resolved })
        }
    }

    /// True if the target has this size and sample count.
    pub fn fits(&self, w: i32, h: i32, samples: i32) -> bool {
        (self.width, self.height, self.samples) == (w, h, samples)
    }

    /// Binds the framebuffer the scene is drawn into and sets the viewport to all of it.
    pub unsafe fn bind_draw(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.ms.as_ref().map_or(self.resolved.fbo, |m| m.fbo)));
            gl.viewport(0, 0, self.width, self.height);
        }
    }

    /// Resolves the multisampled frame into the textures (nothing to do without multisampling).
    pub unsafe fn resolve(&self, gl: &glow::Context) {
        unsafe {
            if let Some(m) = &self.ms {
                gl.bind_framebuffer(glow::READ_FRAMEBUFFER, Some(m.fbo));
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(self.resolved.fbo));
                let (w, h) = (self.width, self.height);
                gl.blit_framebuffer(0, 0, w, h, 0, 0, w, h, glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT, glow::NEAREST);
            }
        }
    }

    /// The resolved colour.
    pub fn color(&self) -> glow::Texture {
        self.resolved.color
    }

    /// The resolved depth.
    pub fn depth(&self) -> Option<glow::Texture> {
        self.resolved.depth
    }

    /// The framebuffer holding the resolved colour and depth (for read-backs).
    pub fn resolved_fbo(&self) -> glow::Framebuffer {
        self.resolved.fbo
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            if let Some(m) = &self.ms {
                m.free(gl);
            }
            self.resolved.free(gl);
        }
    }
}

/// The running average of accumulated frames (float colour, no depth).
pub(super) struct Accumulation {
    target: Attached,
    pub width: i32,
    pub height: i32,
}

impl Accumulation {
    pub unsafe fn new(gl: &glow::Context, w: i32, h: i32) -> Result<Accumulation, String> {
        unsafe { Ok(Accumulation { target: Attached::new(gl, glow::RGBA32F, w, h, false)?, width: w, height: h }) }
    }

    pub fn color(&self) -> glow::Texture {
        self.target.color
    }

    /// Binds the average for drawing a frame into it (with a constant-alpha blend weighing it in).
    pub unsafe fn bind_draw(&self, gl: &glow::Context) {
        unsafe {
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.target.fbo));
            gl.viewport(0, 0, self.width, self.height);
        }
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe { self.target.free(gl) }
    }
}

/// The targets of one view and how far its accumulation got.
#[derive(Default)]
pub(super) struct ViewTargets {
    pub target: Option<HdrTarget>,
    pub accum: Option<Accumulation>,
    pub progress: Progress,
    /// Samples averaged in the accumulation target so far.
    pub averaged: u32,
    /// The plain frame of the current accumulation has everything (it lacks its shadows when they were skipped
    /// while moving): only then does it start the average.
    pub plain_complete: bool,
}

impl ViewTargets {
    /// (Re)creates the targets for a view of `w` × `h` pixels with `samples` samples per pixel; true if they were
    /// made anew.
    pub unsafe fn ensure(&mut self, gl: &glow::Context, w: i32, h: i32, samples: i32) -> Result<bool, String> {
        unsafe {
            if self.target.as_ref().is_some_and(|t| t.fits(w, h, samples)) {
                return Ok(false);
            }
            self.free(gl);
            self.progress.invalidate();
            self.target = Some(HdrTarget::new(gl, w, h, samples)?);
            self.accum = Some(Accumulation::new(gl, w, h)?);
            Ok(true)
        }
    }

    pub unsafe fn free(&mut self, gl: &glow::Context) {
        unsafe {
            if let Some(t) = self.target.take() {
                t.free(gl);
            }
            if let Some(a) = self.accum.take() {
                a.free(gl);
            }
        }
    }
}
