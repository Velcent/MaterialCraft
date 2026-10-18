//! Shader programs: material programs from shader files, the background gradient, the post stage and the
//! accumulation copy, with their uniform locations.
use crate::shading::ShaderFile;
use eframe::egui_glow::glow;
use glow::HasContext;
use std::collections::HashMap;

const BG_VS: &str = include_str!("glsl/background.vert");
const BG_FS: &str = include_str!("glsl/background.frag");
const FULL_VS: &str = include_str!("glsl/fullscreen.vert");
const POST_FS: &str = include_str!("glsl/post.frag");
const ACCUM_FS: &str = include_str!("glsl/accumulate.frag");

const POST_UNIFORMS: &[&str] = &["uSrc", "uOrigin", "uExposure", "uCurve", "uBypass"];
const ACCUM_UNIFORMS: &[&str] = &["uSrc"];

/// A linked program and the locations of the uniforms it uses.
pub(super) struct Program {
    pub id: glow::Program,
    locs: HashMap<String, glow::UniformLocation>,
}

// SAFETY (Program and `compile`): GL calls on the current context (module invariant); a program is deleted only
// in `free`.
impl Program {
    /// The material program of a shader file: the viewer's uniforms ([`crate::shading::decl::RESERVED_UNIFORMS`])
    /// and the parameters it declares.
    pub unsafe fn material(gl: &glow::Context, file: &ShaderFile) -> Result<Program, String> {
        let names = crate::shading::decl::RESERVED_UNIFORMS.iter().copied().chain(file.decl.params.iter().map(|p| p.uniform.as_str()));
        unsafe {
            let program = Program::with_uniforms(gl, &file.vertex_src, &file.fragment_src, names)?;
            if let Err(e) = check_param_types(gl, program.id, file) {
                program.free(gl);
                return Err(e);
            }
            Ok(program)
        }
    }

    /// The post stage: exposure, tone curve and display transform of the HDR frame, blended over the background.
    pub unsafe fn post(gl: &glow::Context) -> Result<Program, String> {
        unsafe { Program::with_uniforms(gl, FULL_VS, POST_FS, POST_UNIFORMS.iter().copied()) }
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
