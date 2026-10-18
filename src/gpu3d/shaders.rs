//! Shader programs: the PBR material program, the background gradient, the post stage and the accumulation copy,
//! with their uniform locations.
use eframe::egui_glow::glow;
use glow::HasContext;
use std::collections::HashMap;

const VS: &str = include_str!("../shading/builtin/pbr.vert");
const FS: &str = include_str!("../shading/builtin/pbr.frag");
const BG_VS: &str = include_str!("glsl/background.vert");
const BG_FS: &str = include_str!("glsl/background.frag");
const FULL_VS: &str = include_str!("glsl/fullscreen.vert");
const POST_FS: &str = include_str!("glsl/post.frag");
const ACCUM_FS: &str = include_str!("glsl/accumulate.frag");

const UNIFORMS: &[&str] = &[
    "uVP",
    "uHeight",
    "uDisp",
    "uLevel",
    "uVtxLod",
    "uBase",
    "uNormal",
    "uRma",
    "uEmis",
    "uEye",
    "uL",
    "uFill",
    "uLightI",
    "uExposure",
    "uAmbient",
    "uNormalAmt",
    "uBump",
    "uDepth",
    "uTexel",
    "uMode",
    "uHasHeight",
    "uHasEmis",
    "uWire",
    "uShadow",
    "uCull",
    "uUnlit",
    "uMipBias",
    "uSceneDepth",
    "uDepthOrigin",
    "uDepthLin",
];
const POST_UNIFORMS: &[&str] = &["uSrc", "uOrigin", "uExposure", "uCurve", "uBypass"];
const ACCUM_UNIFORMS: &[&str] = &["uSrc"];

/// A linked program and the locations of the uniforms it uses.
pub(super) struct Program {
    pub id: glow::Program,
    locs: HashMap<&'static str, glow::UniformLocation>,
}

// SAFETY (Program and `compile`): GL calls on the current context (module invariant); a program is deleted only
// in `free`.
impl Program {
    /// The PBR material program (vertex displacement + GGX shading).
    pub unsafe fn material(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, VS, FS, UNIFORMS) }
    }

    /// The post stage: exposure, tone curve and display transform of the HDR frame, blended over the background.
    pub unsafe fn post(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, FULL_VS, POST_FS, POST_UNIFORMS) }
    }

    /// Copies a frame into the accumulation target (the blend state does the averaging).
    pub unsafe fn accumulate(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, FULL_VS, ACCUM_FS, ACCUM_UNIFORMS) }
    }

    unsafe fn with_uniforms(gl: &glow::Context, vs: &str, fs: &str, names: &[&'static str]) -> Result<Program, String> {
        unsafe {
            let id = compile(gl, vs, fs)?;
            let mut locs = HashMap::new();
            for name in names {
                if let Some(l) = gl.get_uniform_location(id, name) {
                    locs.insert(*name, l);
                }
            }
            Ok(Program { id, locs })
        }
    }

    /// Full-screen background gradient (one triangle, no attributes).
    pub unsafe fn background(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Ok(Program { id: compile(gl, BG_VS, BG_FS)?, locs: HashMap::new() }) }
    }

    pub fn loc(&self, name: &str) -> Option<&glow::UniformLocation> {
        self.locs.get(name)
    }

    pub unsafe fn free(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.id);
        }
    }
}

unsafe fn compile(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let prog = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let sh = gl.create_shader(kind)?;
            gl.shader_source(sh, src);
            gl.compile_shader(sh);
            if !gl.get_shader_compile_status(sh) {
                let log = gl.get_shader_info_log(sh);
                return Err(format!("shader compile failed: {log}"));
            }
            gl.attach_shader(prog, sh);
            shaders.push(sh);
        }
        gl.link_program(prog);
        if !gl.get_program_link_status(prog) {
            return Err(format!("shader link failed: {}", gl.get_program_info_log(prog)));
        }
        for sh in shaders {
            gl.detach_shader(prog, sh);
            gl.delete_shader(sh);
        }
        Ok(prog)
    }
}
