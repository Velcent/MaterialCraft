//! [`ShaderInstance`]: a named set of parameter values for one shader file, plus its displacement and
//! tessellation (VS-03). Texture sets share instances through [`super::ShaderSettings`].

use super::displacement::{Displacement, Tessellation};
use super::file::{DEFAULT_SHADER, ShaderFile};
use super::param::{ParamDecl, ParamValue};

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderInstance {
    /// Unique within the document.
    pub id: String,
    pub name: String,
    /// The shader file's id.
    pub shader: String,
    /// The fingerprint of the shader file the values were made for (VS-05).
    pub fingerprint: u64,
    /// Values by parameter id; a parameter without one has its default. Kept as read: values of parameters the
    /// shader no longer declares stay until the instance is updated.
    pub values: Vec<(String, ParamValue)>,
    pub displacement: Displacement,
    pub tessellation: Tessellation,
}

/// A uniform with the value to send.
#[derive(Debug, Clone, PartialEq)]
pub struct UniformValue {
    pub uniform: String,
    pub value: ParamValue,
}

impl ShaderInstance {
    /// A new instance of `file` with every parameter at its default.
    pub fn new(id: impl Into<String>, name: impl Into<String>, file: &ShaderFile) -> ShaderInstance {
        ShaderInstance {
            id: id.into(),
            name: name.into(),
            shader: file.id().to_string(),
            fingerprint: file.fingerprint,
            values: Vec::new(),
            displacement: Displacement::default(),
            tessellation: Tessellation::default(),
        }
    }

    /// The built-in default shader's instance with defaults: renders exactly as the viewport did before shader
    /// instances.
    pub fn default_of(file: &ShaderFile) -> ShaderInstance {
        ShaderInstance::new("default", "Default", file)
    }

    /// True for an instance of the built-in shader.
    pub fn is_builtin(&self) -> bool {
        self.shader == DEFAULT_SHADER
    }

    /// The stored value of `id`, if any (unchecked).
    pub fn stored(&self, id: &str) -> Option<ParamValue> {
        self.values.iter().find(|(k, _)| k == id).map(|(_, v)| *v)
    }

    /// The value of `decl` (the stored one made valid, or the default).
    pub fn value(&self, decl: &ParamDecl) -> ParamValue {
        match self.stored(&decl.id) {
            Some(v) => decl.sanitize(v),
            None => decl.default_value(),
        }
    }

    /// Sets (`Some`) or clears (`None`: back to the default) the value of `id`; returns the previous stored value.
    pub fn set_value(&mut self, id: &str, value: Option<ParamValue>) -> Option<ParamValue> {
        let at = self.values.iter().position(|(k, _)| k == id);
        match (at, value) {
            (Some(i), Some(v)) => self.values.get_mut(i).map(|slot| std::mem::replace(&mut slot.1, v)),
            (Some(i), None) => Some(self.values.remove(i).1),
            (None, Some(v)) => {
                self.values.push((id.to_string(), v));
                None
            }
            (None, None) => None,
        }
    }

    /// Every declared parameter's uniform with its value, in declaration order.
    pub fn uniforms(&self, file: &ShaderFile) -> Vec<UniformValue> {
        file.decl.params.iter().map(|p| UniformValue { uniform: p.uniform.clone(), value: self.value(p) }).collect()
    }

    /// The shader file changed since the values were made (VS-05).
    pub fn is_outdated(&self, file: &ShaderFile) -> bool {
        self.shader == file.id() && self.fingerprint != file.fingerprint
    }

    /// This instance brought up to `file` (VS-05's update action, or a change of shader file): values of
    /// parameters it doesn't declare are dropped, the rest made valid; displacement and tessellation are kept.
    pub fn updated_to(&self, file: &ShaderFile) -> ShaderInstance {
        let values = self.values.iter().filter_map(|(id, v)| file.decl.param(id).map(|d| (id.clone(), d.sanitize(*v)))).collect();
        ShaderInstance { shader: file.id().to_string(), fingerprint: file.fingerprint, values, ..self.clone() }
    }

    /// This instance with every parameter, the displacement and the tessellation at their defaults (VS-04).
    pub fn with_defaults(&self, file: &ShaderFile) -> ShaderInstance {
        ShaderInstance { id: self.id.clone(), name: self.name.clone(), ..ShaderInstance::new("", "", file) }
    }
}
