//! Brush presets as JSON (our own format, AS-06): every field by name, unknown fields ignored, missing ones
//! taking the default, so presets written by newer versions still load.

use super::preset::{BrushPreset, Jitter, Placement, PressureUse, SizeSpace, Tip};
use crate::json::Value;

/// The format version written into each preset.
/// Version 2 added procedural and alpha-image tips (`tip` names the kind, `tipRef` the tip).
pub const PRESET_VERSION: u64 = 2;

impl BrushPreset {
    pub fn to_json(&self) -> Value {
        let j = &self.jitter;
        let v = Value::object()
            .with("version", PRESET_VERSION)
            .with("name", self.name.as_str())
            .with("size", self.size)
            .with("sizeSpace", self.size_space.id())
            .with("tip", self.tip.kind())
            .with("hardness", self.hardness)
            .with("flow", self.flow)
            .with("opacity", self.opacity)
            .with("spacing", self.spacing)
            .with("angle", self.angle)
            .with("followDirection", self.follow_direction)
            .with("roundness", self.roundness)
            .with("jitter", Value::object().with("size", j.size).with("flow", j.flow).with("angle", j.angle).with("position", j.position))
            .with("pressure", Value::object().with("size", self.pressure.size).with("flow", self.pressure.flow))
            .with("placement", self.placement.id())
            .with("maxAngle", self.max_angle);
        match self.tip.reference() {
            Some(r) => v.with("tipRef", r),
            None => v,
        }
    }

    /// Reads a preset; missing or unreadable fields keep the default's value. `None` if `v` is not an object.
    pub fn from_json(v: &Value) -> Option<BrushPreset> {
        v.as_object()?;
        let d = BrushPreset::default();
        let num = |v: &Value, def: f32| v.as_f64().map_or(def, |n| n as f32);
        let flag = |v: &Value, def: bool| v.as_bool().unwrap_or(def);
        let j = field(v, "jitter");
        let p = field(v, "pressure");
        Some(BrushPreset {
            name: field(v, "name").as_str().map_or(d.name.clone(), str::to_string),
            size: num(field(v, "size"), d.size),
            size_space: field(v, "sizeSpace").as_str().and_then(SizeSpace::from_id).unwrap_or(d.size_space),
            tip: field(v, "tip").as_str().and_then(|k| Tip::from_parts(k, field(v, "tipRef").as_str())).unwrap_or_default(),
            hardness: num(field(v, "hardness"), d.hardness),
            flow: num(field(v, "flow"), d.flow),
            opacity: num(field(v, "opacity"), d.opacity),
            spacing: num(field(v, "spacing"), d.spacing),
            angle: num(field(v, "angle"), d.angle),
            follow_direction: flag(field(v, "followDirection"), d.follow_direction),
            roundness: num(field(v, "roundness"), d.roundness),
            jitter: Jitter {
                size: num(field(j, "size"), 0.0),
                flow: num(field(j, "flow"), 0.0),
                angle: num(field(j, "angle"), 0.0),
                position: num(field(j, "position"), 0.0),
            },
            pressure: PressureUse { size: flag(field(p, "size"), d.pressure.size), flow: flag(field(p, "flow"), d.pressure.flow) },
            placement: field(v, "placement").as_str().and_then(Placement::from_id).unwrap_or(d.placement),
            max_angle: num(field(v, "maxAngle"), d.max_angle),
        })
    }
}

/// Member `key` of `v`, `Null` when missing.
pub(super) fn field<'a>(v: &'a Value, key: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    v.get(key).unwrap_or(&NULL)
}
