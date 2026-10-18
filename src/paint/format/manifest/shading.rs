//! Shader settings (since 0.13): the root's `shader_instances` and `mesh_unit`, and each texture set's `shader`
//! (written by [`assign_sets`], read by [`super::texture_set`] into [`ReadCtx::set_shaders`]).
//!
//! ```json
//! "shader_instances": [{
//!   "id": "shader", "name": "Skin", "shader": "materialcraft.pbr", "fingerprint": "8c4f…",
//!   "values": { "ao_strength": 0.5, "base_tint": [1, 0.9, 0.8], "sss": true },
//!   "displacement": { "enabled": true, "source": "height", "scale": 0.02, "unit": "relative", "midlevel": 0 },
//!   "tessellation": { "mode": "uniform", "levels": 2, "max_edge": 0.01, "smooth": false }
//! }],
//! "mesh_unit": "cm"
//! ```
//!
//! Values are numbers, booleans or `[r, g, b]`; whole and fractional numbers are told apart by the shader file's
//! declaration when the value is used. Unknown fields are kept at `shader_instances/<id>`,
//! `…/displacement` and `…/tessellation`.

use super::extras::{Extras, loc};
use super::fields::{Obj, as_f32, num};
use super::{FormatError, ReadCtx};
use crate::json::Value;
use crate::mesh::LengthUnit;
use crate::shading::{Displacement, DisplacementSource, DisplacementUnit, ParamValue, ShaderInstance, ShaderSettings, Tessellation, TessellationMode};

const HERE: &str = "shader_instances";

fn value_json(v: ParamValue) -> Value {
    match v {
        ParamValue::Float(f) => num(f),
        ParamValue::Int(i) => Value::from(i),
        ParamValue::Bool(b) => Value::Bool(b),
        ParamValue::Color(c) => c.into_iter().map(num).collect(),
    }
}

fn read_value(v: &Value) -> Option<ParamValue> {
    match v {
        Value::Bool(b) => Some(ParamValue::Bool(*b)),
        Value::Number(_) => as_f32(v).map(ParamValue::Float),
        Value::Array(a) if a.len() == 3 => {
            let mut c = [0.0; 3];
            for (o, x) in c.iter_mut().zip(a) {
                *o = as_f32(x)?;
            }
            Some(ParamValue::Color(c))
        }
        _ => None,
    }
}

fn instance_json(i: &ShaderInstance, extras: &Extras) -> Value {
    let here = loc("", HERE, &i.id);
    let d = &i.displacement;
    let mut disp = Value::object()
        .with("enabled", d.enabled)
        .with("source", d.source.id())
        .with("scale", num(d.scale))
        .with("unit", d.unit.id())
        .with("midlevel", num(d.midlevel));
    extras.inject(&format!("{here}/displacement"), &mut disp);
    let t = &i.tessellation;
    let mut tess = match t.mode {
        TessellationMode::Off => Value::object().with("mode", "off"),
        TessellationMode::Uniform { levels } => Value::object().with("mode", "uniform").with("levels", levels),
        TessellationMode::EdgeLength { max_relative } => Value::object().with("mode", "edge_length").with("max_edge", num(max_relative)),
    };
    tess.insert("smooth", t.smooth);
    extras.inject(&format!("{here}/tessellation"), &mut tess);
    let values = Value::Object(i.values.iter().map(|(k, v)| (k.clone(), value_json(*v))).collect());
    let mut v = Value::object()
        .with("id", i.id.as_str())
        .with("name", i.name.as_str())
        .with("shader", i.shader.as_str())
        .with("fingerprint", format!("{:016x}", i.fingerprint))
        .with("values", values)
        .with("displacement", disp)
        .with("tessellation", tess);
    extras.inject(&here, &mut v);
    v
}

/// Adds the root fields of `s` to `root` (nothing for empty settings, so older documents are written as before).
pub fn write(s: &ShaderSettings, root: &mut Value, extras: &Extras) {
    if !s.instances.is_empty() {
        root.insert(HERE, s.instances.iter().map(|i| instance_json(i, extras)).collect::<Value>());
    }
    if let Some(u) = s.mesh_unit {
        root.insert("mesh_unit", u.id());
    }
}

/// Writes each texture set's `shader` into the written `sets` (assignments to sets or instances that don't exist
/// are left out).
pub fn assign_sets(s: &ShaderSettings, sets: &mut Value) {
    let Some(sets) = sets.as_array_mut() else { return };
    for set in sets {
        let id = set.get("id").and_then(Value::as_str).map(str::to_string);
        if let Some(instance) = id.and_then(|id| s.assigned(&id).filter(|i| s.instance(i).is_some()).map(str::to_string)) {
            set.insert("shader", instance);
        }
    }
}

fn read_displacement(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Displacement, FormatError> {
    let d = Displacement::default();
    let mut o = Obj::new(v, at)?;
    let source = match o.opt_str("source")? {
        None => d.source,
        Some(id) => DisplacementSource::from_id(id).ok_or_else(|| o.error("source", format!("unknown displacement source {id:?}")))?,
    };
    let unit = match o.opt_str("unit")? {
        None => d.unit,
        Some(id) => DisplacementUnit::from_id(id).ok_or_else(|| o.error("unit", format!("unknown displacement unit {id:?}")))?,
    };
    let out = Displacement {
        enabled: o.bool_or("enabled", d.enabled)?,
        source,
        scale: o.f32_or("scale", d.scale)?,
        unit,
        midlevel: o.f32_or("midlevel", d.midlevel)?,
    };
    ctx.extras.record(&format!("{here}/displacement"), &o);
    Ok(out)
}

fn read_tessellation(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Tessellation, FormatError> {
    let mut o = Obj::new(v, at)?;
    let mode = match o.opt_str("mode")?.unwrap_or("off") {
        "off" => TessellationMode::Off,
        "uniform" => {
            let levels = match o.opt("levels") {
                None => 1,
                Some(l) => l.as_u64().and_then(|l| u32::try_from(l).ok()).ok_or_else(|| o.error("levels", "expected a non-negative integer"))?,
            };
            TessellationMode::Uniform { levels }
        }
        "edge_length" => TessellationMode::EdgeLength { max_relative: o.f32_or("max_edge", 0.01)? },
        other => return Err(o.error("mode", format!("unknown tessellation mode {other:?}"))),
    };
    let out = Tessellation { mode, smooth: o.bool_or("smooth", false)? };
    ctx.extras.record(&format!("{here}/tessellation"), &o);
    Ok(out)
}

fn read_instance(index: usize, v: &Value, ctx: &mut ReadCtx) -> Result<ShaderInstance, FormatError> {
    let mut o = Obj::new(v, format!("{HERE}[{index}]"))?;
    let id = o.str("id")?.to_string();
    let here = loc("", HERE, &id);
    let fingerprint = match o.opt_str("fingerprint")? {
        None => 0,
        Some(hex) => u64::from_str_radix(hex, 16).map_err(|_| o.error("fingerprint", "expected a hexadecimal number"))?,
    };
    let mut values = Vec::new();
    for (k, v) in o.entries("values")? {
        let value = read_value(v).ok_or_else(|| o.error("values", format!("{k:?}: expected a number, true/false or [r, g, b]")))?;
        values.push((k.clone(), value));
    }
    let displacement = match o.opt("displacement") {
        None => Displacement::default(),
        Some(d) => read_displacement(d, o.path("displacement"), &here, ctx)?,
    };
    let tessellation = match o.opt("tessellation") {
        None => Tessellation::default(),
        Some(t) => read_tessellation(t, o.path("tessellation"), &here, ctx)?,
    };
    let instance = ShaderInstance {
        name: o.opt_str("name")?.unwrap_or(&id).to_string(),
        shader: o.str("shader")?.to_string(),
        id,
        fingerprint,
        values,
        displacement,
        tessellation,
    };
    ctx.extras.record(&here, &o);
    Ok(instance)
}

/// Reads the root's `shader_instances` and `mesh_unit`; the assignments come from the texture sets
/// ([`ReadCtx::set_shaders`]) and are added here (those naming no instance are dropped).
pub fn read(root: &mut Obj, ctx: &mut ReadCtx) -> Result<ShaderSettings, FormatError> {
    let mut s = ShaderSettings::default();
    for (i, v) in root.array(HERE)?.iter().enumerate() {
        let instance = read_instance(i, v, ctx)?;
        if s.instance(&instance.id).is_some() {
            return Err(FormatError::manifest(format!("{HERE}[{i}].id"), format!("{:?} is used twice", instance.id)));
        }
        s.instances.push(instance);
    }
    s.mesh_unit = match root.opt_str("mesh_unit")? {
        None => None,
        Some(id) => Some(LengthUnit::from_id(id).ok_or_else(|| root.error("mesh_unit", format!("unknown length unit {id:?}")))?),
    };
    for (set, instance) in std::mem::take(&mut ctx.set_shaders) {
        if s.instance(&instance).is_some() {
            s.assign(&set, Some(instance));
        }
    }
    Ok(s)
}
