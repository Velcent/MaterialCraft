//! The fields of the effect kinds that are the same in content and mask stacks:
//!
//! - generator, filter: `"id"`, `"params"` (name → string, in order), and `"original"` (`{"id", "params"}`)
//!   when it stands in for another procedural;
//! - levels: `"input_min"`, `"input_max"`, `"gamma"`, `"output_min"`, `"output_max"` (3 numbers each), `"clamp"`;
//! - color_selection: `"map"` (baked map kind), `"colors"` (arrays of 3 numbers), `"tolerance"`, `"hardness"`;
//! - polygon_fill: `"mode"` and `"picks"` (`[{"part", "triangles"}]`), plus the values of the stack's fills;
//! - anchor: `"id"`; reference: `"anchor"`, optional `"channel"` (a channel id) and `"levels"` (fields as levels).

use super::ReadCtx;
use super::extras::{Extras, loc};
use super::fields::{Obj, as_f32, num};
use crate::json::Value;
use crate::paint::effect::{Anchor, ColorSelection, Levels, Pick, PickMode, Procedural, Reference, Selection, UnknownEffect};
use crate::paint::format::FormatError;
use crate::paint::{BakedKind, Channel};

/// Effect kinds of this version; any other kind is kept as an [`UnknownEffect`].
pub const KNOWN: [&str; 9] = ["paint", "fill", "generator", "filter", "levels", "color_selection", "polygon_fill", "anchor", "reference"];

pub fn write_procedural(p: &Procedural, v: &mut Value) {
    let params = p.params.iter().map(|(k, val)| (k.clone(), Value::from(val.as_str()))).collect();
    v.insert("id", p.id.as_str());
    v.insert("params", Value::Object(params));
    if let Some(original) = &p.original {
        let mut o = Value::object();
        write_procedural(original, &mut o);
        v.insert("original", o);
    }
}

pub fn read_procedural(o: &mut Obj) -> Result<Procedural, FormatError> {
    let id = o.str("id")?.to_string();
    let mut params = Vec::new();
    for (k, val) in o.entries("params")? {
        let val = val.as_str().ok_or_else(|| o.error("params", format!("{k:?}: expected a string")))?;
        params.push((k.clone(), val.to_string()));
    }
    let original = match o.opt("original") {
        Some(v) => Some(Box::new(read_procedural(&mut Obj::new(v, o.path("original"))?)?)),
        None => None,
    };
    Ok(Procedural { id, params, original })
}

pub fn write_levels(l: &Levels, v: &mut Value) {
    let three = |a: [f32; 3]| Value::Array(a.into_iter().map(num).collect());
    v.insert("input_min", three(l.input_min));
    v.insert("input_max", three(l.input_max));
    v.insert("gamma", three(l.gamma));
    v.insert("output_min", three(l.output_min));
    v.insert("output_max", three(l.output_max));
    v.insert("clamp", l.clamp);
}

pub fn read_levels(o: &mut Obj) -> Result<Levels, FormatError> {
    let d = Levels::default();
    Ok(Levels {
        input_min: three_or(o, "input_min", d.input_min)?,
        input_max: three_or(o, "input_max", d.input_max)?,
        gamma: three_or(o, "gamma", d.gamma)?,
        output_min: three_or(o, "output_min", d.output_min)?,
        output_max: three_or(o, "output_max", d.output_max)?,
        clamp: o.bool_or("clamp", d.clamp)?,
    })
}

pub fn write_color_selection(c: &ColorSelection, v: &mut Value) {
    v.insert("map", c.map.id());
    v.insert("colors", c.colors.iter().map(|rgb| Value::Array(rgb.iter().map(|x| num(*x)).collect())).collect::<Value>());
    v.insert("tolerance", num(c.tolerance));
    v.insert("hardness", num(c.hardness));
}

pub fn read_color_selection(o: &mut Obj) -> Result<ColorSelection, FormatError> {
    let map = BakedKind::from_id(o.opt_str("map")?.unwrap_or("id"));
    let mut colors = Vec::new();
    for c in o.array("colors")? {
        colors.push(three(c).ok_or_else(|| o.error("colors", "expected arrays of 3 numbers"))?);
    }
    Ok(ColorSelection { map, colors, tolerance: o.f32_or("tolerance", 0.0)?, hardness: o.f32_or("hardness", 1.0)? })
}

/// Writes `"mode"` and `"picks"`; `here` is the effect's extras location.
pub fn write_selection(s: &Selection, here: &str, extras: &Extras, v: &mut Value) {
    let picks: Value = s
        .picks
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut pv = Value::object().with("part", p.part.as_str()).with("triangles", p.triangles.clone());
            extras.inject(&loc(here, "picks", &i.to_string()), &mut pv);
            pv
        })
        .collect();
    v.insert("mode", s.mode.id());
    v.insert("picks", picks);
}

pub fn read_selection(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<Selection, FormatError> {
    let mode_id = o.opt_str("mode")?.unwrap_or("triangle");
    let mode = PickMode::from_id(mode_id).ok_or_else(|| o.error("mode", format!("unknown pick mode {mode_id:?}")))?;
    let mut picks = Vec::new();
    for (i, p) in o.array("picks")?.iter().enumerate() {
        let mut po = Obj::new(p, format!("{}[{i}]", o.path("picks")))?;
        let part = po.str("part")?.to_string();
        let triangles: Option<Vec<u32>> = po.array("triangles")?.iter().map(|t| t.as_u64().and_then(|t| u32::try_from(t).ok())).collect();
        let triangles = triangles.ok_or_else(|| po.error("triangles", "expected triangle indices"))?;
        ctx.extras.record(&loc(here, "picks", &i.to_string()), &po);
        picks.push(Pick { part, triangles });
    }
    Ok(Selection { mode, picks })
}

pub fn read_anchor(o: &mut Obj) -> Result<Anchor, FormatError> {
    Ok(Anchor { id: o.str("id")?.to_string() })
}

pub fn write_reference(r: &Reference, v: &mut Value) {
    v.insert("anchor", r.anchor.as_str());
    if let Some(c) = r.channel {
        v.insert("channel", c.id());
    }
    if let Some(l) = &r.levels {
        let mut lv = Value::object();
        write_levels(l, &mut lv);
        v.insert("levels", lv);
    }
}

pub fn read_reference(o: &mut Obj) -> Result<Reference, FormatError> {
    let anchor = o.str("anchor")?.to_string();
    let channel = match o.opt_str("channel")? {
        Some(id) => Some(Channel::from_id(id).ok_or_else(|| o.error("channel", format!("unknown channel {id:?}")))?),
        None => None,
    };
    let levels = match o.opt("levels") {
        Some(v) => Some(read_levels(&mut Obj::new(v, o.path("levels"))?)?),
        None => None,
    };
    Ok(Reference { anchor, channel, levels })
}

/// An unknown effect as it was read (its kind first if the data lost it).
pub fn write_unknown(u: &UnknownEffect) -> Value {
    let mut v = Value::object().with("kind", u.kind.as_str());
    for (k, val) in u.data.as_object().into_iter().flatten().filter(|(k, _)| k != "kind") {
        v.insert(k.clone(), val.clone());
    }
    v
}

fn three(v: &Value) -> Option<[f32; 3]> {
    match v.as_array()?.as_slice() {
        [a, b, c] => Some([as_f32(a)?, as_f32(b)?, as_f32(c)?]),
        _ => None,
    }
}

fn three_or(o: &mut Obj, key: &str, default: [f32; 3]) -> Result<[f32; 3], FormatError> {
    match o.opt(key) {
        None => Ok(default),
        Some(v) => three(v).ok_or_else(|| o.error(key, "expected 3 numbers")),
    }
}
