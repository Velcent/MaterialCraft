//! How scene parts are shaded: the shader file whose program draws a part, the values of its parameters and the
//! instance's displacement ([`PartShading`]), and the compiled programs ([`Programs`]).
use super::shaders::Program;
use crate::shading::{ParamValue, ShaderFile, ShaderInstance, UniformValue};
use eframe::egui_glow::glow;
use glow::HasContext;
use std::sync::{Arc, OnceLock};

/// A shader instance's displacement as the viewport uses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InstanceDisplacement {
    /// How far height 1 lies above `level`, in viewport units (the model fitted to unit size).
    pub distance: f32,
    /// The height that stays on the surface.
    pub level: f32,
}

/// What a part is drawn with.
#[derive(Debug, Clone, PartialEq)]
pub struct PartShading {
    pub shader: Arc<ShaderFile>,
    /// Every declared parameter's uniform and value.
    pub uniforms: Vec<UniformValue>,
    /// `None`: the view's own height display applies (as before shader instances).
    pub displacement: Option<InstanceDisplacement>,
}

impl Default for PartShading {
    /// The built-in default shader with its defaults: the look of the viewport before shader files.
    fn default() -> Self {
        let shader = builtin();
        let uniforms = ShaderInstance::default_of(&shader).uniforms(&shader);
        PartShading { shader, uniforms, displacement: None }
    }
}

impl PartShading {
    /// `instance` drawn with `shader` (its file, or the built-in one when the file is missing).
    pub fn of(instance: &ShaderInstance, shader: Arc<ShaderFile>, displacement: Option<InstanceDisplacement>) -> PartShading {
        PartShading { uniforms: instance.uniforms(&shader), shader, displacement }
    }
}

/// The built-in shader file, parsed once per process.
pub fn builtin() -> Arc<ShaderFile> {
    static BUILTIN: OnceLock<Arc<ShaderFile>> = OnceLock::new();
    BUILTIN.get_or_init(|| Arc::new(ShaderFile::builtin())).clone()
}

/// The material programs: the built-in one, compiled when the viewer starts (a failure stops the viewer, as
/// before), and those of other shader files, compiled when first needed. A shader file that doesn't compile is
/// drawn with the built-in program; its error is reported once.
pub(super) struct Programs {
    builtin_key: u64,
    builtin: Program,
    others: Vec<(u64, Result<Program, String>)>,
    errors: Vec<String>,
}

// SAFETY: GL calls on the current context (module invariant); programs are deleted only in `free`.
impl Programs {
    pub unsafe fn new(gl: &glow::Context) -> Result<Programs, String> {
        let file = builtin();
        let program = unsafe { Program::material(gl, &file)? };
        Ok(Programs { builtin_key: file.program_key, builtin: program, others: Vec::new(), errors: Vec::new() })
    }

    /// The built-in program.
    pub fn builtin(&self) -> &Program {
        &self.builtin
    }

    /// Compiles the programs of `files` that are not compiled yet.
    pub unsafe fn prepare<'a>(&mut self, gl: &glow::Context, files: impl IntoIterator<Item = &'a ShaderFile>) {
        for file in files {
            if file.program_key == self.builtin_key || self.others.iter().any(|(k, _)| *k == file.program_key) {
                continue;
            }
            let result = unsafe { Program::material(gl, file) };
            if let Err(e) = &result {
                self.errors.push(format!("The shader {:?} can't be used ({e}); its parts are drawn with the built-in shader.", file.decl.name));
            }
            self.others.push((file.program_key, result));
        }
    }

    /// The program for `file` (the built-in one if it isn't compiled or failed).
    pub fn get(&self, file: &ShaderFile) -> &Program {
        match self.others.iter().find(|(k, _)| *k == file.program_key) {
            Some((_, Ok(p))) => p,
            _ => &self.builtin,
        }
    }

    /// Errors not reported yet.
    pub fn take_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }

    pub unsafe fn free(self, gl: &glow::Context) {
        unsafe {
            self.builtin.free(gl);
            for (_, p) in self.others.into_iter() {
                if let Ok(p) = p {
                    p.free(gl);
                }
            }
        }
    }
}

/// Sends declared parameter values (uniforms the program doesn't use are skipped).
// SAFETY: the program is bound on the current context (module invariant).
pub(super) unsafe fn send_uniforms(gl: &glow::Context, prog: &Program, uniforms: &[UniformValue]) {
    unsafe {
        for u in uniforms {
            let loc = prog.loc(&u.uniform);
            match u.value {
                ParamValue::Float(v) => gl.uniform_1_f32(loc, v),
                ParamValue::Int(v) => gl.uniform_1_i32(loc, v),
                ParamValue::Bool(b) => gl.uniform_1_i32(loc, b as i32),
                ParamValue::Color([r, g, b]) => gl.uniform_3_f32(loc, r, g, b),
            }
        }
    }
}
