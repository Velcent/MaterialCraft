//! A fill's `"projection"` object (since 0.4): `{"mode", "filter", "repeat", "image": {…}}` plus the members of
//! the mode (`volume`, `clip_to_volume`, `blend_hardness`, `depth_falloff`, `facing_falloff`, `coverage_angle`,
//! `hide_caps`, `warp`) and, for the volume-based modes, the fill symmetry (`symmetry`: `{"kind": "mirror", "axis",
//! "offset"}` or `{"kind": "radial", "axis", "point", "count", "flip": "none"|"u"|"v"}`, axes 0 = x … 2 = z; left out
//! without symmetry). Written only when not the default (texture space).
//!
//! An unknown value of `mode`, `filter`, `repeat` or `size_mode` (a newer file) does not make the file unreadable
//! (ADR 0007): the projection loads as [`ProjectionMode::Unknown`] holding the whole object, which is written back
//! unchanged and is not computable.

use super::ReadCtx;
use super::extras::Extras;
use super::fields::{Obj, as_f32, num};
use crate::json::{self, Value};
use crate::paint::format::FormatError;
use crate::projection::{
    FacingFalloff, Falloff, Filter, ImagePlacement, Projection, ProjectionMode, Repeat, SizeMode, UnknownMode, Volume, WarpGrid, WarpPoint,
};
use crate::symmetry::{Flip, SymmetrySpec};

/// The projection object, or `None` for the default.
pub fn write(p: &Projection, here: &str, extras: &Extras) -> Option<Value> {
    if p.is_default() {
        return None;
    }
    if let ProjectionMode::Unknown(u) = &p.mode {
        return Some(json::parse(&u.json).unwrap_or_else(|_| Value::object().with("mode", u.mode.as_str())));
    }
    let mut v = Value::object()
        .with("mode", p.mode.id())
        .with("filter", p.filter.id())
        .with("repeat", p.repeat.id())
        .with("image", write_image(&p.image, here, extras));
    match &p.mode {
        ProjectionMode::TriPlanar(m) => {
            write_volume(&mut v, &m.volume, here, extras);
            v.insert("blend_hardness", num(m.blend_hardness));
        }
        ProjectionMode::Planar(m) => {
            write_volume(&mut v, &m.volume, here, extras);
            v.insert("depth_falloff", falloff(&m.depth_falloff, &format!("{here}/depth_falloff"), extras));
            v.insert("facing_falloff", facing(&m.facing_falloff, &format!("{here}/facing_falloff"), extras));
        }
        ProjectionMode::Spherical(m) => write_volume(&mut v, &m.volume, here, extras),
        ProjectionMode::Cylindrical(m) => {
            write_volume(&mut v, &m.volume, here, extras);
            v.insert("facing_falloff", facing(&m.facing_falloff, &format!("{here}/facing_falloff"), extras));
            v.insert("coverage_angle", num(m.coverage_angle));
            v.insert("hide_caps", m.hide_caps);
        }
        ProjectionMode::Warp(m) => {
            write_volume(&mut v, &m.volume, here, extras);
            v.insert("depth_falloff", falloff(&m.depth_falloff, &format!("{here}/depth_falloff"), extras));
            v.insert("warp", write_grid(&m.grid, &format!("{here}/warp"), extras));
        }
        ProjectionMode::TextureSpace | ProjectionMode::Uv | ProjectionMode::Unknown(_) => {}
    }
    if let Some(sym) = write_symmetry(p.symmetry, &format!("{here}/symmetry"), extras) {
        v.insert("symmetry", sym);
    }
    extras.inject(here, &mut v);
    Some(v)
}

fn flip_id(f: Flip) -> &'static str {
    match f {
        Flip::None => "none",
        Flip::U => "u",
        Flip::V => "v",
    }
}

fn write_symmetry(s: SymmetrySpec, here: &str, extras: &Extras) -> Option<Value> {
    let mut v = match s {
        SymmetrySpec::None => return None,
        SymmetrySpec::Mirror { axis, offset } => Value::object().with("kind", "mirror").with("axis", axis as u64).with("offset", num(offset)),
        SymmetrySpec::Radial { axis, point, count, flip } => {
            Value::object().with("kind", "radial").with("axis", axis as u64).with("point", nums(&point)).with("count", count).with("flip", flip_id(flip))
        }
    };
    extras.inject(here, &mut v);
    Some(v)
}

fn nums(v: &[f32]) -> Value {
    Value::Array(v.iter().copied().map(num).collect())
}

fn write_image(p: &ImagePlacement, here: &str, extras: &Extras) -> Value {
    let mut v =
        Value::object().with("tiling", nums(&p.tiling)).with("rotation", num(p.rotation)).with("offset", nums(&p.offset)).with("size_mode", p.size_mode.id());
    if let Some(size) = p.physical_size {
        v.insert("physical_size", nums(&size));
    }
    extras.inject(&format!("{here}/image"), &mut v);
    v
}

fn write_volume(v: &mut Value, vol: &Volume, here: &str, extras: &Extras) {
    let mut o = Value::object().with("offset", nums(&vol.offset)).with("rotation", nums(&vol.rotation)).with("scale", nums(&vol.scale));
    extras.inject(&format!("{here}/volume"), &mut o);
    v.insert("volume", o);
    v.insert("clip_to_volume", vol.clip);
}

fn falloff(f: &Falloff, here: &str, extras: &Extras) -> Value {
    let mut v = Value::object().with("on", f.on).with("hardness", num(f.hardness));
    extras.inject(here, &mut v);
    v
}

fn facing(f: &FacingFalloff, here: &str, extras: &Extras) -> Value {
    let mut v = Value::object().with("on", f.on).with("angle", num(f.angle)).with("hardness", num(f.hardness));
    extras.inject(here, &mut v);
    v
}

fn write_grid(g: &WarpGrid, here: &str, extras: &Extras) -> Value {
    let points = g
        .points
        .iter()
        .map(|p| Value::object().with("pos", nums(&p.pos)).with("uv", nums(&p.uv)).with("tangents", Value::Array(p.tangents.iter().map(|t| nums(t)).collect())))
        .collect();
    let mut v = Value::object().with("rows", g.rows).with("columns", g.columns).with("depth", num(g.depth)).with("points", Value::Array(points));
    extras.inject(here, &mut v);
    v
}

/// Why a projection object is read as [`ProjectionMode::Unknown`] rather than rejected.
struct Unknown(String);

/// Reads the projection object at `at` (its extras location `here`).
pub fn read(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Projection, FormatError> {
    let mut o = Obj::new(v, at)?;
    match read_known(&mut o, here, ctx)? {
        Ok(p) => {
            ctx.extras.record(here, &o);
            Ok(p)
        }
        Err(Unknown(mode)) => Ok(Projection { mode: ProjectionMode::Unknown(UnknownMode { mode, json: v.to_string() }), ..Projection::default() }),
    }
}

fn read_known(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<Result<Projection, Unknown>, FormatError> {
    let mode_id = o.str("mode")?.to_string();
    let unknown = || Ok(Err(Unknown(mode_id.clone())));
    let Some(mut mode) = ProjectionMode::default_of(&mode_id) else { return unknown() };
    let Some(filter) = enum_or(o, "filter", Filter::Smooth, Filter::from_id)? else { return unknown() };
    let Some(repeat) = enum_or(o, "repeat", Repeat::Both, Repeat::from_id)? else { return unknown() };
    let image = match o.opt("image") {
        Some(v) => match read_image(v, o.path("image"), &format!("{here}/image"), ctx)? {
            Some(i) => i,
            None => return unknown(),
        },
        None => ImagePlacement::default(),
    };
    match &mut mode {
        ProjectionMode::TriPlanar(m) => {
            m.volume = read_volume(o, here, ctx)?;
            m.blend_hardness = o.f32_or("blend_hardness", 0.5)?;
        }
        ProjectionMode::Planar(m) => {
            m.volume = read_volume(o, here, ctx)?;
            m.depth_falloff = read_falloff(o, "depth_falloff", here, ctx)?;
            m.facing_falloff = read_facing(o, here, ctx)?;
        }
        ProjectionMode::Spherical(m) => m.volume = read_volume(o, here, ctx)?,
        ProjectionMode::Cylindrical(m) => {
            m.volume = read_volume(o, here, ctx)?;
            m.facing_falloff = read_facing(o, here, ctx)?;
            m.coverage_angle = o.f32_or("coverage_angle", 360.0)?;
            m.hide_caps = o.bool_or("hide_caps", false)?;
        }
        ProjectionMode::Warp(m) => {
            m.volume = read_volume(o, here, ctx)?;
            m.depth_falloff = read_falloff(o, "depth_falloff", here, ctx)?;
            if let Some(g) = o.opt("warp") {
                m.grid = read_grid(g, o.path("warp"), &format!("{here}/warp"), ctx)?;
            }
        }
        ProjectionMode::TextureSpace | ProjectionMode::Uv | ProjectionMode::Unknown(_) => {}
    }
    let symmetry = match o.opt("symmetry") {
        Some(v) => match read_symmetry(v, o.path("symmetry"), &format!("{here}/symmetry"), ctx)? {
            Some(s) => s,
            None => return unknown(),
        },
        None => SymmetrySpec::None,
    };
    Ok(Ok(Projection { mode, filter, repeat, image, symmetry }))
}

/// The symmetry object; `Ok(None)` for a kind or flip this version does not know.
fn read_symmetry(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Option<SymmetrySpec>, FormatError> {
    let mut o = Obj::new(v, at)?;
    let kind = o.str("kind")?.to_string();
    let axis = match o.opt("axis") {
        Some(v) => v.as_u64().filter(|a| *a < 3).ok_or_else(|| o.error("axis", "expected 0, 1 or 2"))? as usize,
        None => 0,
    };
    let spec = match kind.as_str() {
        "mirror" => SymmetrySpec::Mirror { axis, offset: o.f32_or("offset", 0.0)? },
        "radial" => {
            let Some(flip) = enum_or(&mut o, "flip", Flip::None, |id| Flip::ALL.into_iter().find(|f| flip_id(*f) == id))? else { return Ok(None) };
            let count = match o.opt("count") {
                Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| o.error("count", "expected a non-negative integer"))?,
                None => 2,
            };
            SymmetrySpec::Radial { axis, point: nums_or(&mut o, "point", [0.0; 3])?, count, flip }
        }
        _ => return Ok(None),
    };
    ctx.extras.record(here, &o);
    Ok(Some(spec))
}

/// An enum field: `Ok(Some(default))` when missing, `Ok(None)` for an id this version does not know.
fn enum_or<T>(o: &mut Obj, key: &str, default: T, parse: fn(&str) -> Option<T>) -> Result<Option<T>, FormatError> {
    match o.opt_str(key)? {
        None => Ok(Some(default)),
        Some(id) => Ok(parse(id)),
    }
}

/// `N` numbers at `key`, or `default` when missing.
fn nums_or<const N: usize>(o: &mut Obj, key: &str, default: [f32; N]) -> Result<[f32; N], FormatError> {
    let Some(v) = o.opt(key) else { return Ok(default) };
    let read: Option<Vec<f32>> = v.as_array().filter(|a| a.len() == N).and_then(|a| a.iter().map(as_f32).collect());
    read.and_then(|r| <[f32; N]>::try_from(r).ok()).ok_or_else(|| o.error(key, format!("expected {N} numbers")))
}

fn read_image(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Option<ImagePlacement>, FormatError> {
    let mut o = Obj::new(v, at)?;
    let d = ImagePlacement::default();
    let Some(size_mode) = enum_or(&mut o, "size_mode", SizeMode::Tiling, SizeMode::from_id)? else { return Ok(None) };
    let physical_size = match o.opt("physical_size") {
        Some(_) => Some(nums_or(&mut o, "physical_size", [0.0; 2])?),
        None => None,
    };
    let p = ImagePlacement {
        tiling: nums_or(&mut o, "tiling", d.tiling)?,
        rotation: o.f32_or("rotation", d.rotation)?,
        offset: nums_or(&mut o, "offset", d.offset)?,
        size_mode,
        physical_size,
    };
    ctx.extras.record(here, &o);
    Ok(Some(p))
}

fn read_volume(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<Volume, FormatError> {
    let d = Volume::default();
    let clip = o.bool_or("clip_to_volume", d.clip)?;
    let Some(v) = o.opt("volume") else { return Ok(Volume { clip, ..d }) };
    let mut vo = Obj::new(v, o.path("volume"))?;
    let vol = Volume {
        offset: nums_or(&mut vo, "offset", d.offset)?,
        rotation: nums_or(&mut vo, "rotation", d.rotation)?,
        scale: nums_or(&mut vo, "scale", d.scale)?,
        clip,
    };
    ctx.extras.record(&format!("{here}/volume"), &vo);
    Ok(vol)
}

fn read_falloff(o: &mut Obj, key: &str, here: &str, ctx: &mut ReadCtx) -> Result<Falloff, FormatError> {
    let d = Falloff::default();
    let Some(v) = o.opt(key) else { return Ok(d) };
    let mut fo = Obj::new(v, o.path(key))?;
    let f = Falloff { on: fo.bool_or("on", d.on)?, hardness: fo.f32_or("hardness", d.hardness)? };
    ctx.extras.record(&format!("{here}/{key}"), &fo);
    Ok(f)
}

fn read_facing(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<FacingFalloff, FormatError> {
    let d = FacingFalloff::default();
    let Some(v) = o.opt("facing_falloff") else { return Ok(d) };
    let mut fo = Obj::new(v, o.path("facing_falloff"))?;
    let f = FacingFalloff { on: fo.bool_or("on", d.on)?, angle: fo.f32_or("angle", d.angle)?, hardness: fo.f32_or("hardness", d.hardness)? };
    ctx.extras.record(&format!("{here}/facing_falloff"), &fo);
    Ok(f)
}

fn read_grid(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<WarpGrid, FormatError> {
    let mut o = Obj::new(v, at)?;
    let d = WarpGrid::default();
    let count = |o: &mut Obj, key: &str, default: u32| -> Result<u32, FormatError> {
        match o.opt(key) {
            None => Ok(default),
            Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| o.error(key, "expected a non-negative integer")),
        }
    };
    let rows = count(&mut o, "rows", d.rows)?;
    let columns = count(&mut o, "columns", d.columns)?;
    let depth = o.f32_or("depth", d.depth)?;
    let mut points = Vec::new();
    let list = o.array("points")?;
    for (i, p) in list.iter().enumerate() {
        let mut po = Obj::new(p, format!("{}[{i}]", o.path("points")))?;
        let tangents: Vec<[f32; 3]> = match po.opt("tangents") {
            None => vec![[0.0; 3]; 4],
            Some(t) => {
                let parsed: Option<Vec<[f32; 3]>> = t.as_array().filter(|a| a.len() == 4).and_then(|a| {
                    a.iter()
                        .map(|t| {
                            t.as_array()
                                .filter(|t| t.len() == 3)
                                .and_then(|t| t.iter().map(as_f32).collect::<Option<Vec<f32>>>())
                                .and_then(|t| <[f32; 3]>::try_from(t).ok())
                        })
                        .collect()
                });
                parsed.ok_or_else(|| po.error("tangents", "expected 4 triples of numbers"))?
            }
        };
        let tangents = <[[f32; 3]; 4]>::try_from(tangents).map_err(|_| po.error("tangents", "expected 4 triples of numbers"))?;
        points.push(WarpPoint { pos: nums_or(&mut po, "pos", [0.0; 3])?, uv: nums_or(&mut po, "uv", [0.0; 2])?, tangents });
    }
    ctx.extras.record(here, &o);
    Ok(WarpGrid { rows, columns, depth, points })
}

/// One example of every mode with some settings changed, for round-trip tests.
#[cfg(test)]
pub(crate) fn all_modes() -> Vec<ProjectionMode> {
    use crate::projection::{Cylindrical, Planar, Spherical, TriPlanar, Warp};
    vec![
        ProjectionMode::TextureSpace,
        ProjectionMode::Uv,
        ProjectionMode::TriPlanar(TriPlanar { blend_hardness: 0.8, ..TriPlanar::default() }),
        ProjectionMode::Planar(Planar { depth_falloff: Falloff { on: true, hardness: 0.75 }, ..Planar::default() }),
        ProjectionMode::Spherical(Spherical::default()),
        ProjectionMode::Cylindrical(Cylindrical { hide_caps: true, ..Cylindrical::default() }),
        ProjectionMode::Warp(Warp {
            grid: WarpGrid { points: vec![WarpPoint { pos: [0.5, -0.5, 0.0], uv: [1.0, 0.0], tangents: [[0.1, 0.0, 0.0]; 4] }], ..WarpGrid::default() },
            ..Warp::default()
        }),
    ]
}
