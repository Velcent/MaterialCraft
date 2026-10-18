//! Shader programs: the PBR material program, the shadow depth pass, the background, the post stage, the
//! accumulation copy and the studio bake, with their uniform locations. Shared GLSL (the environment lookups, the
//! studio, the tone stage) is put in front of the programs that use it.
use crate::shading::ShaderFile;
use eframe::egui_glow::glow;
use glow::HasContext;
use std::collections::HashMap;

const VERSION: &str = "#version 330 core\n";
const SHADOW_FS: &str = include_str!("glsl/shadow.frag");
const BG_VS: &str = include_str!("glsl/background.vert");
const BG_FS: &str = include_str!("glsl/background.frag");
const FULL_VS: &str = include_str!("glsl/fullscreen.vert");
const POST_FS: &str = include_str!("glsl/post.frag");
const ACCUM_FS: &str = include_str!("glsl/accumulate.frag");
const STUDIO_BAKE_FS: &str = include_str!("glsl/studio_bake.frag");
// shared parts
const ENVIRONMENT: &str = include_str!("glsl/environment.glsl");
const STUDIO: &str = include_str!("glsl/studio.glsl");
const TONE: &str = include_str!("glsl/tone.glsl");

const POST_UNIFORMS: &[&str] = &["uSrc", "uOrigin", "uExposure", "uCurve", "uBypass"];
const ACCUM_UNIFORMS: &[&str] = &["uSrc"];
const BG_UNIFORMS: &[&str] =
    &["uInvVP", "uOpacity", "uBlur", "uExposure", "uCurve", "uStudioKey", "uKey", "uEnvSharp", "uEnvSpec", "uEnvDiff", "uEnvLevels", "uEnvRot"];
const STUDIO_BAKE_UNIFORMS: &[&str] = &["uSize", "uRough", "uDiffuse"];

/// A linked program and the locations of the uniforms it uses.
pub(super) struct Program {
    pub id: glow::Program,
    locs: HashMap<String, glow::UniformLocation>,
}

// SAFETY (Program and `compile`): GL calls on the current context (module invariant); a program is deleted only
// in `free`.
impl Program {
    /// The material program of a shader file: the viewer's uniforms ([`crate::shading::decl::RESERVED_UNIFORMS`])
    /// and the parameters it declares. The fragment stage gets the environment lookups and the studio
    /// (`environment.glsl`, `studio.glsl`) after its `#version` line.
    pub unsafe fn material(gl: &glow::Context, file: &ShaderFile) -> Result<Program, String> {
        let names = crate::shading::decl::RESERVED_UNIFORMS.iter().copied().chain(file.decl.params.iter().map(|p| p.uniform.as_str()));
        let fragment = with_library(&file.fragment_src);
        unsafe {
            let program = Program::with_uniforms(gl, &file.vertex_src, &fragment, names)?;
            if let Err(e) = check_param_types(gl, program.id, file) {
                program.free(gl);
                return Err(e);
            }
            Ok(program)
        }
    }

    /// The shadow pass: the built-in shader's vertices (displaced the same way), depth only.
    pub unsafe fn shadow(gl: &glow::Context) -> Result<Program, String> {
        let file = super::shading::builtin();
        unsafe { Program::with_uniforms(gl, &file.vertex_src, &[VERSION, SHADOW_FS].concat(), crate::shading::decl::RESERVED_UNIFORMS.iter().copied()) }
    }

    /// The post stage: exposure, tone curve and display transform of the HDR frame, blended over the background.
    pub unsafe fn post(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, FULL_VS, &[VERSION, TONE, POST_FS].concat(), POST_UNIFORMS.iter().copied()) }
    }

    /// Bakes the studio into environment levels and its diffuse image.
    pub unsafe fn studio_bake(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, FULL_VS, &[VERSION, STUDIO, STUDIO_BAKE_FS].concat(), STUDIO_BAKE_UNIFORMS.iter().copied()) }
    }

    /// Copies a frame into the accumulation target (the blend state does the averaging).
    pub unsafe fn accumulate(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, FULL_VS, ACCUM_FS, ACCUM_UNIFORMS.iter().copied()) }
    }

    unsafe fn with_uniforms<'n>(gl: &glow::Context, vs: &str, fs: &str, names: impl IntoIterator<Item = &'n str>) -> Result<Program, String> {
        unsafe {
            let id = compile(gl, vs, fs)?;
            let mut locs = HashMap::new();
            for name in names {
                if let Some(l) = gl.get_uniform_location(id, name) {
                    locs.insert(name.to_string(), l);
                }
            }
            Ok(Program { id, locs })
        }
    }

    /// Full-screen background: the gradient and the environment over it (one triangle, no attributes).
    pub unsafe fn background(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, &[VERSION, BG_VS].concat(), &[VERSION, TONE, ENVIRONMENT, STUDIO, BG_FS].concat(), BG_UNIFORMS.iter().copied()) }
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

/// A material fragment source with the shared environment lookups and studio after its `#version` line (in front
/// of it, with a `#version` of its own, when the source has none).
fn with_library(src: &str) -> String {
    let (head, body) = match src.trim_start().strip_prefix("#version") {
        Some(_) => {
            let start = src.len() - src.trim_start().len();
            let end = src[start..].find('\n').map_or(src.len(), |i| start + i + 1);
            (src[..end].to_string(), &src[end..])
        }
        None => (VERSION.to_string(), src),
    };
    [head.as_str(), if head.ends_with('\n') { "" } else { "\n" }, ENVIRONMENT, STUDIO, body].concat()
}

/// Checks that every declared parameter the program uses has the GLSL type its kind is sent as (a mismatch would
/// make the uniform calls fail while drawing).
// SAFETY: `id` is a linked program on the current context.
unsafe fn check_param_types(gl: &glow::Context, id: glow::Program, file: &ShaderFile) -> Result<(), String> {
    use crate::shading::ParamKind;
    unsafe {
        for i in 0..gl.get_active_uniforms(id) {
            let Some(u) = gl.get_active_uniform(id, i) else { continue };
            let Some(p) = file.decl.params.iter().find(|p| p.uniform == u.name) else { continue };
            let ok = match p.kind {
                ParamKind::Float { .. } => u.utype == glow::FLOAT,
                ParamKind::Int { .. } | ParamKind::Choice { .. } => u.utype == glow::INT,
                ParamKind::Bool { .. } => u.utype == glow::BOOL || u.utype == glow::INT,
                ParamKind::Color { .. } => u.utype == glow::FLOAT_VEC3,
            };
            if !ok || u.size != 1 {
                return Err(format!("the uniform {} doesn't have the type its parameter {:?} declares", u.name, p.id));
            }
        }
        Ok(())
    }
}

/// Compiles and links a program. On failure every object made so far is deleted (shader files from users may not
/// compile, and the viewer goes on with the built-in program).
unsafe fn compile(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let prog = gl.create_program()?;
        let mut shaders = Vec::new();
        let mut result = Ok(());
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let sh = match gl.create_shader(kind) {
                Ok(sh) => sh,
                Err(e) => {
                    result = Err(e);
                    break;
                }
            };
            gl.shader_source(sh, src);
            gl.compile_shader(sh);
            gl.attach_shader(prog, sh);
            shaders.push(sh);
            if !gl.get_shader_compile_status(sh) {
                result = Err(format!("shader compile failed: {}", gl.get_shader_info_log(sh)));
                break;
            }
        }
        if result.is_ok() {
            gl.link_program(prog);
            if !gl.get_program_link_status(prog) {
                result = Err(format!("shader link failed: {}", gl.get_program_info_log(prog)));
            }
        }
        for sh in shaders {
            gl.detach_shader(prog, sh);
            gl.delete_shader(sh);
        }
        match result {
            Ok(()) => Ok(prog),
            Err(e) => {
                gl.delete_program(prog);
                Err(e)
            }
        }
    }
}
