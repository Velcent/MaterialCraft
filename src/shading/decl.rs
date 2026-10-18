//! [`ShaderDecl`]: the JSON half of a shader file: its id, name, GLSL file names and declared parameters.
//!
//! ```json
//! {
//!   "format": "mcshader", "format_version": 1,
//!   "id": "materialcraft.pbr", "name": "PBR", "revision": 1,
//!   "vertex": "pbr.vert", "fragment": "pbr.frag",
//!   "params": [
//!     { "id": "ao_strength", "label": "AO strength", "group": "Surface", "uniform": "uAoStrength",
//!       "type": "float", "min": 0, "max": 1, "default": 0.85 },
//!     { "id": "tint", "label": "Tint", "uniform": "uTint", "type": "color", "default": [1, 1, 1] },
//!     { "id": "sss", "label": "Subsurface", "uniform": "uSss", "type": "bool", "default": false },
//!     { "id": "mode", "label": "Mode", "uniform": "uM", "type": "choice", "options": ["A", "B"], "default": "A" }
//!   ]
//! }
//! ```
//!
//! Parameter types: `float` (`min`, `max`, `default`, optional `step`), `int` (`min`, `max`, `default`), `bool`,
//! `color` (linear RGB, components 0 … 1) and `choice` (`options`, `default` naming one; the shader gets its
//! index). `group` and `label` are optional (default: no group, the id). Uniform names must be GLSL identifiers,
//! unique, and not one of the names the viewer sets itself ([`RESERVED_UNIFORMS`]).

use super::param::{ParamDecl, ParamKind};
use crate::json::{self, Value};
use std::fmt;

/// Value of the `format` field.
pub const FORMAT: &str = "mcshader";
/// The declaration format version this code reads and writes.
pub const FORMAT_VERSION: u64 = 1;

/// The uniforms the viewer sets on every material program (the program interface every shader file implements).
/// A parameter may not use one of them.
pub const RESERVED_UNIFORMS: &[&str] = &[
    "uVP",
    "uHeight",
    "uDisp",
    "uLevel",
    "uDispLevel",
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
    // environment and scene shadows (step 39); the environment lookups and the studio are added to every
    // fragment stage after its #version line (src/gpu3d/glsl/environment.glsl, studio.glsl)
    "uEnvSharp",
    "uEnvSpec",
    "uEnvDiff",
    "uEnvLevels",
    "uEnvRot",
    "uEnvScale",
    "uStudioKey",
    "uShadowMode",
    "uShadowMap",
    "uShadowVP",
    "uShadowOpacity",
    "uShadowOffset",
    "uDomCount",
    "uDomDir",
    "uDomE",
    // the 2D view (step 34): the mesh laid out flat in UV space
    "uUvSpace",
];

/// A parsed, checked declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct ShaderDecl {
    pub id: String,
    pub name: String,
    /// Bumped by the shader's author when parameters change meaning (instances of an older revision are outdated).
    pub revision: u32,
    /// GLSL file names, relative to the declaration.
    pub vertex: String,
    pub fragment: String,
    pub params: Vec<ParamDecl>,
}

/// Why a declaration was refused: the JSON path and what is wrong there.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclError {
    pub at: String,
    pub message: String,
}

impl fmt::Display for DeclError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.at.is_empty() { write!(f, "{}", self.message) } else { write!(f, "{}: {}", self.at, self.message) }
    }
}

impl std::error::Error for DeclError {}

fn err(at: impl Into<String>, message: impl Into<String>) -> DeclError {
    DeclError { at: at.into(), message: message.into() }
}

impl ShaderDecl {
    /// Parses and checks a declaration.
    pub fn parse(text: &str) -> Result<ShaderDecl, DeclError> {
        let root = json::parse(text).map_err(|e| err("", format!("not JSON: {e}")))?;
        ShaderDecl::from_json(&root)
    }

    pub fn from_json(root: &Value) -> Result<ShaderDecl, DeclError> {
        if root.as_object().is_none() {
            return Err(err("", "expected an object"));
        }
        if root.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(err("format", format!("expected {FORMAT:?}")));
        }
        match root.get("format_version").and_then(Value::as_u64) {
            Some(v) if v <= FORMAT_VERSION => {}
            Some(v) => return Err(err("format_version", format!("version {v} is newer than this program reads ({FORMAT_VERSION})"))),
            None => return Err(err("format_version", "expected a whole number")),
        }
        let id = string(root, "id", "")?;
        if id.is_empty() {
            return Err(err("id", "is empty"));
        }
        let revision = match root.get("revision") {
            None => 1,
            Some(v) => v.as_u64().and_then(|r| u32::try_from(r).ok()).ok_or_else(|| err("revision", "expected a whole number"))?,
        };
        let params = match root.get("params") {
            None => Vec::new(),
            Some(v) => v
                .as_array()
                .ok_or_else(|| err("params", "expected an array"))?
                .iter()
                .enumerate()
                .map(|(i, p)| param(p, &format!("params[{i}]")))
                .collect::<Result<_, _>>()?,
        };
        let decl = ShaderDecl {
            name: opt_string(root, "name", "")?.unwrap_or_else(|| id.clone()),
            id,
            revision,
            vertex: string(root, "vertex", "")?,
            fragment: string(root, "fragment", "")?,
            params,
        };
        decl.check()?;
        Ok(decl)
    }

    /// Unique ids and uniforms, none of them reserved.
    fn check(&self) -> Result<(), DeclError> {
        for (i, p) in self.params.iter().enumerate() {
            let at = format!("params[{i}]");
            if self.params.iter().take(i).any(|q| q.id == p.id) {
                return Err(err(format!("{at}.id"), format!("{:?} is declared twice", p.id)));
            }
            if self.params.iter().take(i).any(|q| q.uniform == p.uniform) {
                return Err(err(format!("{at}.uniform"), format!("{:?} is used twice", p.uniform)));
            }
        }
        Ok(())
    }

    /// The parameter with `id`.
    pub fn param(&self, id: &str) -> Option<&ParamDecl> {
        self.params.iter().find(|p| p.id == id)
    }

    /// The groups in declaration order (each once).
    pub fn groups(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for p in &self.params {
            if !out.contains(&p.group.as_str()) {
                out.push(&p.group);
            }
        }
        out
    }

    /// The declaration as JSON (what [`Self::parse`] reads back).
    pub fn to_json(&self) -> Value {
        let params: Value = self.params.iter().map(param_json).collect();
        Value::object()
            .with("format", FORMAT)
            .with("format_version", FORMAT_VERSION)
            .with("id", self.id.as_str())
            .with("name", self.name.as_str())
            .with("revision", self.revision)
            .with("vertex", self.vertex.as_str())
            .with("fragment", self.fragment.as_str())
            .with("params", params)
    }
}

fn param_json(p: &ParamDecl) -> Value {
    let v = Value::object().with("id", p.id.as_str()).with("label", p.label.as_str()).with("group", p.group.as_str()).with("uniform", p.uniform.as_str());
    match &p.kind {
        ParamKind::Float { min, max, default, step } => {
            let v = v.with("type", "float").with("min", n(*min)).with("max", n(*max)).with("default", n(*default));
            match step {
                Some(s) => v.with("step", n(*s)),
                None => v,
            }
        }
        ParamKind::Int { min, max, default } => v.with("type", "int").with("min", *min).with("max", *max).with("default", *default),
        ParamKind::Bool { default } => v.with("type", "bool").with("default", *default),
        ParamKind::Color { default } => v.with("type", "color").with("default", default.iter().map(|c| n(*c)).collect::<Value>()),
        ParamKind::Choice { options, default } => {
            let name = options.get(*default).cloned().unwrap_or_default();
            v.with("type", "choice").with("options", options.iter().map(String::as_str).collect::<Value>()).with("default", name)
        }
    }
}

/// `x` as the shortest decimal that reads back as the same `f32` (0.85, not 0.8500000238418579).
fn n(x: f32) -> Value {
    Value::Number(x.to_string().parse::<f64>().unwrap_or(0.0))
}

fn path(at: &str, key: &str) -> String {
    if at.is_empty() { key.to_string() } else { format!("{at}.{key}") }
}

fn string(o: &Value, key: &str, at: &str) -> Result<String, DeclError> {
    opt_string(o, key, at)?.ok_or_else(|| err(path(at, key), "missing"))
}

fn opt_string(o: &Value, key: &str, at: &str) -> Result<Option<String>, DeclError> {
    match o.get(key) {
        None => Ok(None),
        Some(v) => v.as_str().map(|s| Some(s.to_string())).ok_or_else(|| err(path(at, key), "expected a string")),
    }
}

fn number(o: &Value, key: &str, at: &str) -> Result<f32, DeclError> {
    let v = o.get(key).ok_or_else(|| err(path(at, key), "missing"))?;
    let n = v.as_f64().map(|n| n as f32).filter(|n| n.is_finite()).ok_or_else(|| err(path(at, key), "expected a finite number"))?;
    Ok(n)
}

fn whole(o: &Value, key: &str, at: &str) -> Result<i32, DeclError> {
    let v = o.get(key).ok_or_else(|| err(path(at, key), "missing"))?;
    v.as_i64().and_then(|n| i32::try_from(n).ok()).ok_or_else(|| err(path(at, key), "expected a whole number"))
}

/// A GLSL identifier: a letter or `_` first, then letters, digits and `_`; no `gl_` prefix, no double underscore.
pub fn is_glsl_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    first_ok && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !s.starts_with("gl_") && !s.contains("__")
}

fn param(p: &Value, at: &str) -> Result<ParamDecl, DeclError> {
    if p.as_object().is_none() {
        return Err(err(at, "expected an object"));
    }
    let id = string(p, "id", at)?;
    if id.is_empty() {
        return Err(err(path(at, "id"), "is empty"));
    }
    let uniform = string(p, "uniform", at)?;
    if !is_glsl_identifier(&uniform) {
        return Err(err(path(at, "uniform"), format!("{uniform:?} is not a GLSL identifier")));
    }
    if RESERVED_UNIFORMS.contains(&uniform.as_str()) {
        return Err(err(path(at, "uniform"), format!("{uniform:?} is set by the viewer")));
    }
    let kind = match string(p, "type", at)?.as_str() {
        "float" => {
            let (min, max) = (number(p, "min", at)?, number(p, "max", at)?);
            if min >= max {
                return Err(err(path(at, "max"), "must be greater than min"));
            }
            let default = number(p, "default", at)?;
            if !(min..=max).contains(&default) {
                return Err(err(path(at, "default"), "outside min … max"));
            }
            let step = match p.get("step") {
                None => None,
                Some(_) => Some(number(p, "step", at)?).filter(|s| *s > 0.0).map(Some).ok_or_else(|| err(path(at, "step"), "must be positive"))?,
            };
            ParamKind::Float { min, max, default, step }
        }
        "int" => {
            let (min, max) = (whole(p, "min", at)?, whole(p, "max", at)?);
            if min >= max {
                return Err(err(path(at, "max"), "must be greater than min"));
            }
            let default = whole(p, "default", at)?;
            if !(min..=max).contains(&default) {
                return Err(err(path(at, "default"), "outside min … max"));
            }
            ParamKind::Int { min, max, default }
        }
        "bool" => {
            ParamKind::Bool { default: p.get("default").map_or(Some(false), Value::as_bool).ok_or_else(|| err(path(at, "default"), "expected true or false"))? }
        }
        "color" => {
            let bad = || err(path(at, "default"), "expected [r, g, b] with components 0 … 1");
            let a = p.get("default").and_then(Value::as_array).filter(|a| a.len() == 3).ok_or_else(bad)?;
            let mut default = [0.0f32; 3];
            for (d, v) in default.iter_mut().zip(a) {
                *d = v.as_f64().map(|n| n as f32).filter(|n| (0.0..=1.0).contains(n)).ok_or_else(bad)?;
            }
            ParamKind::Color { default }
        }
        "choice" => {
            let bad = || err(path(at, "options"), "expected a non-empty array of distinct names");
            let options: Vec<String> = p
                .get("options")
                .and_then(Value::as_array)
                .ok_or_else(bad)?
                .iter()
                .map(|o| o.as_str().map(str::to_string))
                .collect::<Option<_>>()
                .ok_or_else(bad)?;
            if options.is_empty() || options.iter().enumerate().any(|(i, o)| options.iter().take(i).any(|q| q == o)) {
                return Err(bad());
            }
            let default = match p.get("default") {
                None => 0,
                Some(v) => {
                    v.as_str().and_then(|d| options.iter().position(|o| o == d)).ok_or_else(|| err(path(at, "default"), "expected one of the options"))?
                }
            };
            ParamKind::Choice { options, default }
        }
        other => return Err(err(path(at, "type"), format!("unknown parameter type {other:?}"))),
    };
    Ok(ParamDecl {
        label: opt_string(p, "label", at)?.unwrap_or_else(|| id.clone()),
        group: opt_string(p, "group", at)?.unwrap_or_default(),
        id,
        uniform,
        kind,
    })
}
