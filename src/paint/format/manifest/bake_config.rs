//! Bake configuration (since 0.5): a texture set's `"bake"` object and the document's `"bake_links"`.
//!
//! ```text
//! "bake": {
//!   "common": {"size", "max_dist", "rear_dist"?, "padding", "samples", "average_normals", "ignore_backfaces",
//!              "high": {"low_as_high", "files": [..], "placement"?: {"center": [x, y, z], "radius"}}},
//!   "maps": {"<map>": {"include", "show", "auto_rebake", <the baker's settings>}, …}
//! }
//! "bake_links": {"common": bool, "maps": ["<map>", …]}
//! ```
//!
//! Every field is optional on reading (missing = the default); sets without `"bake"` get
//! [`SetBakeConfig::with_size`] of their resolution.

use super::bake_enums::EnumId;
use super::extras::loc;
use super::fields::{Obj, as_f32, num};
use super::{ReadCtx, WriteCtx};
use crate::bake::{CurvatureRange, MapKind, MapSelection};
use crate::json::Value;
use crate::paint::bake_config::{BakeLinks, CommonBake, HighPlacement, HighPoly, MapFlags, MapSettings, SetBakeConfig};
use crate::paint::format::FormatError;

pub fn write(c: &SetBakeConfig, here: &str, ctx: &WriteCtx) -> Value {
    let mut maps = Value::object();
    for entry in c.maps.iter() {
        let kind = entry.kind();
        let mloc = loc(here, "maps", kind.name());
        let f = entry.flags;
        let mut v = Value::object().with("include", f.include).with("show", f.show).with("auto_rebake", f.auto_rebake);
        write_settings(entry.settings(), &mut v);
        ctx.extras.inject(&mloc, &mut v);
        maps.insert(kind.name(), v);
    }
    let mut v = Value::object().with("common", write_common(&c.common, here, ctx)).with("maps", maps);
    ctx.extras.inject(here, &mut v);
    v
}

fn write_common(c: &CommonBake, here: &str, ctx: &WriteCtx) -> Value {
    let mut high = Value::object().with("low_as_high", c.high.low_as_high).with("files", c.high.files.iter().map(String::as_str).collect::<Value>());
    if let Some(p) = c.high.placement {
        high.insert("placement", Value::object().with("center", Value::Array(p.center.into_iter().map(num).collect())).with("radius", num(p.radius)));
    }
    ctx.extras.inject(&format!("{here}/common/high"), &mut high);
    let mut v = Value::object()
        .with("size", c.size)
        .with("max_dist", num(c.max_dist))
        .with("padding", c.padding)
        .with("samples", c.samples)
        .with("average_normals", c.average_normals)
        .with("ignore_backfaces", c.ignore_backfaces)
        .with("high", high);
    if let Some(r) = c.rear_dist {
        v.insert("rear_dist", num(r));
    }
    ctx.extras.inject(&format!("{here}/common"), &mut v);
    v
}

fn write_settings(s: &MapSettings, v: &mut Value) {
    match s {
        MapSettings::Normal { tangents } => {
            v.insert("tangents", tangents.id());
        }
        MapSettings::WorldNormal | MapSettings::Id => {}
        MapSettings::Position(p) => {
            v.insert("mode", p.mode.id());
            v.insert("bounds", p.bounds.id());
        }
        MapSettings::AmbientOcclusion { rays, distance, ao } => {
            v.insert("rays", *rays);
            v.insert("distance", num(*distance));
            v.insert("attenuation", ao.attenuation.id());
            v.insert("distribution", ao.distribution.id());
            v.insert("spread", num(ao.spread));
            v.insert("min_dist", num(ao.min_dist));
        }
        MapSettings::Curvature(c) => {
            v.insert("radius", num(c.radius));
            v.insert("rays", c.rays);
            match c.range {
                CurvatureRange::Auto => v.insert("range", "auto"),
                CurvatureRange::Fixed(r) => v.insert("range", num(r)),
            };
        }
        MapSettings::Thickness(t) => {
            v.insert("rays", t.rays);
            v.insert("max_dist", num(t.max_dist));
            v.insert("distribution", t.distribution.id());
            v.insert("maximise_range", t.maximise_range);
        }
    }
}

pub fn write_links(links: &BakeLinks) -> Value {
    Value::object().with("common", links.common).with("maps", links.maps.kinds().map(|k| Value::from(k.name())).collect::<Value>())
}

/// A set's `"bake"`; `default` when it is missing.
pub fn read(v: Option<&Value>, at: String, here: &str, default: SetBakeConfig, ctx: &mut ReadCtx) -> Result<SetBakeConfig, FormatError> {
    let mut c = default;
    let Some(v) = v else { return Ok(c) };
    let mut o = Obj::new(v, at)?;
    if let Some(common) = o.opt("common") {
        c.common = read_common(common, o.path("common"), here, c.common, ctx)?;
    }
    let maps = o.entries("maps")?;
    for (name, mv) in maps {
        let path = format!("{}.{name}", o.path("maps"));
        let Some(kind) = MapKind::from_name(name) else {
            return Err(FormatError::manifest(path, format!("unknown map {name:?}")));
        };
        let mut mo = Obj::new(mv, path)?;
        let d = MapFlags::default();
        *c.maps.flags_mut(kind) =
            MapFlags { include: mo.bool_or("include", d.include)?, show: mo.bool_or("show", d.show)?, auto_rebake: mo.bool_or("auto_rebake", d.auto_rebake)? };
        let settings = read_settings(&mut mo, c.maps.get(kind).settings().clone())?;
        c.maps.set_settings(settings);
        ctx.extras.record(&loc(here, "maps", name), &mo);
    }
    ctx.extras.record(here, &o);
    Ok(c)
}

fn read_common(v: &Value, at: String, here: &str, d: CommonBake, ctx: &mut ReadCtx) -> Result<CommonBake, FormatError> {
    let mut o = Obj::new(v, at)?;
    let high = match o.opt("high") {
        Some(h) => read_high(h, o.path("high"), &format!("{here}/common/high"), ctx)?,
        None => d.high,
    };
    let rear_dist = match o.opt("rear_dist") {
        None => None,
        Some(r) => Some(as_f32(r).ok_or_else(|| o.error("rear_dist", "expected a number"))?),
    };
    let c = CommonBake {
        size: u32_or(&mut o, "size", d.size)?,
        max_dist: o.f32_or("max_dist", d.max_dist)?,
        rear_dist,
        padding: u32_or(&mut o, "padding", d.padding)?,
        samples: u32_or(&mut o, "samples", d.samples)?,
        average_normals: o.bool_or("average_normals", d.average_normals)?,
        ignore_backfaces: o.bool_or("ignore_backfaces", d.ignore_backfaces)?,
        high,
    };
    ctx.extras.record(&format!("{here}/common"), &o);
    Ok(c)
}

fn read_high(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<HighPoly, FormatError> {
    let mut o = Obj::new(v, at)?;
    let files: Option<Vec<String>> = o.array("files")?.iter().map(|f| f.as_str().map(str::to_string)).collect();
    let files = files.ok_or_else(|| o.error("files", "expected an array of file names"))?;
    let placement = match o.opt("placement") {
        None => None,
        Some(p) => {
            let mut po = Obj::new(p, o.path("placement"))?;
            let center: Option<Vec<f32>> = po.req("center")?.as_array().and_then(|a| a.iter().map(as_f32).collect());
            let center = match center.as_deref() {
                Some(&[x, y, z]) => [x, y, z],
                _ => return Err(po.error("center", "expected [x, y, z]")),
            };
            let radius = po.f32_or("radius", 1.0)?;
            Some(HighPlacement { center, radius })
        }
    };
    let high = HighPoly { low_as_high: o.bool_or("low_as_high", false)?, files, placement };
    ctx.extras.record(here, &o);
    Ok(high)
}

fn read_settings(o: &mut Obj, d: MapSettings) -> Result<MapSettings, FormatError> {
    Ok(match d {
        MapSettings::Normal { tangents } => MapSettings::Normal { tangents: enum_or(o, "tangents", tangents)? },
        MapSettings::WorldNormal => MapSettings::WorldNormal,
        MapSettings::Id => MapSettings::Id,
        MapSettings::Position(mut p) => {
            p.mode = enum_or(o, "mode", p.mode)?;
            p.bounds = enum_or(o, "bounds", p.bounds)?;
            MapSettings::Position(p)
        }
        MapSettings::AmbientOcclusion { rays, distance, mut ao } => {
            ao.attenuation = enum_or(o, "attenuation", ao.attenuation)?;
            ao.distribution = enum_or(o, "distribution", ao.distribution)?;
            ao.spread = o.f32_or("spread", ao.spread)?;
            ao.min_dist = o.f32_or("min_dist", ao.min_dist)?;
            MapSettings::AmbientOcclusion { rays: u32_or(o, "rays", rays)?, distance: o.f32_or("distance", distance)?, ao }
        }
        MapSettings::Curvature(mut c) => {
            c.radius = o.f32_or("radius", c.radius)?;
            c.rays = u32_or(o, "rays", c.rays)?;
            c.range = match o.opt("range") {
                None => c.range,
                Some(v) if v.as_str() == Some("auto") => CurvatureRange::Auto,
                Some(v) => CurvatureRange::Fixed(as_f32(v).ok_or_else(|| o.error("range", "expected \"auto\" or a number"))?),
            };
            MapSettings::Curvature(c)
        }
        MapSettings::Thickness(mut t) => {
            t.rays = u32_or(o, "rays", t.rays)?;
            t.max_dist = o.f32_or("max_dist", t.max_dist)?;
            t.distribution = enum_or(o, "distribution", t.distribution)?;
            t.maximise_range = o.bool_or("maximise_range", t.maximise_range)?;
            MapSettings::Thickness(t)
        }
    })
}

/// The document's `"bake_links"` (missing = nothing linked).
pub fn read_links(v: Option<&Value>, at: String, ctx: &mut ReadCtx) -> Result<BakeLinks, FormatError> {
    let Some(v) = v else { return Ok(BakeLinks::default()) };
    let mut o = Obj::new(v, at)?;
    let mut maps = MapSelection::none();
    for name in o.array("maps")? {
        let kind = name.as_str().and_then(MapKind::from_name).ok_or_else(|| o.error("maps", format!("unknown map {name}")))?;
        maps = maps.with(kind);
    }
    let links = BakeLinks { common: o.bool_or("common", false)?, maps };
    ctx.extras.record("bake_links", &o);
    Ok(links)
}

fn u32_or(o: &mut Obj, key: &str, default: u32) -> Result<u32, FormatError> {
    match o.opt(key) {
        None => Ok(default),
        Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| o.error(key, "expected a non-negative integer")),
    }
}

fn enum_or<T: EnumId>(o: &mut Obj, key: &str, default: T) -> Result<T, FormatError> {
    match o.opt_str(key)? {
        None => Ok(default),
        Some(id) => T::from_id(id).ok_or_else(|| o.error(key, format!("unknown value {id:?}"))),
    }
}
