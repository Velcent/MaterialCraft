//! Layers: `{"id", "name", "kind", "visible", "opacity", "blend", "mask", "geometry_mask", …}` plus `images`
//! (paint), `fill` (fill) or `children` (group), and `effects` (the content effects, since 0.3).

use super::extras::{Extras, loc};
use super::fields::{Obj, as_f32, num, num4};
use super::{ReadCtx, WriteCtx, blend, effect, effect_mask, geometry_mask, image_ref};
use crate::json::Value;
use crate::paint::format::FormatError;
use crate::paint::format::slot::ImageSlot;
use crate::paint::{Channel, ChannelSpec, ColorSpace, Fill, FillSource, Layer, LayerImage, LayerKind, PaintContent};

pub use blend::channel;

/// The texture set a layer belongs to, while writing or reading.
pub struct SetScope<'a> {
    pub index: usize,
    pub loc: &'a str,
    pub channels: &'a [ChannelSpec],
}

impl SetScope<'_> {
    pub fn color_space(&self, c: Channel) -> ColorSpace {
        self.channels.iter().find(|s| s.channel == c).map_or(c.default_color_space(), |s| s.color_space)
    }
}

pub fn write_all(layers: &[Layer], set: &SetScope, ctx: &WriteCtx) -> Value {
    layers.iter().map(|l| write(l, set, ctx)).collect()
}

fn write(l: &Layer, set: &SetScope, ctx: &WriteCtx) -> Value {
    let here = loc(set.loc, "layers", &l.id);
    let kind = match l.kind {
        LayerKind::Paint(_) => "paint",
        LayerKind::Fill(_) => "fill",
        LayerKind::Group(_) => "group",
    };
    let mut v = Value::object()
        .with("id", l.id.as_str())
        .with("name", l.name.as_str())
        .with("kind", kind)
        .with("visible", l.visible)
        .with("opacity", num(l.opacity))
        .with("blend", blend::write_channels(&l.blend, &here, ctx.extras));
    if let Some(m) = &l.mask {
        v.insert("mask", effect_mask::write(m, set, &l.id, &format!("{here}/mask"), ctx));
    }
    if let Some(g) = &l.geometry_mask {
        v.insert("geometry_mask", geometry_mask::write(g, &format!("{here}/geometry_mask"), ctx));
    }
    match &l.kind {
        LayerKind::Paint(p) => {
            let mut images = Value::object();
            for (c, img) in &p.images {
                let slot = ImageSlot::Layer { set: set.index, layer: l.id.clone(), image: LayerImage::Paint(*c) };
                let path = ctx.plan.path(&slot).unwrap_or_default();
                let r = image_ref::write(path, img, set.color_space(*c), &loc(&here, "images", &c.id()), ctx.extras);
                images.insert(c.id(), r);
            }
            v.insert("images", images);
        }
        LayerKind::Fill(f) => {
            v.insert("fill", write_fill(f, &format!("{here}/fill"), ctx.extras));
        }
        LayerKind::Group(children) => {
            v.insert("children", write_all(children, set, ctx));
        }
    }
    if !l.effects.is_empty() {
        v.insert("effects", effect::write_all(&l.effects, set, &l.id, &here, ctx));
    }
    ctx.extras.inject(&here, &mut v);
    v
}

/// A fill object: `{"values"}` or `{"material", "params"}`.
pub fn write_fill(f: &Fill, here: &str, extras: &Extras) -> Value {
    let mut v = match &f.source {
        FillSource::Uniform(values) => {
            let mut obj = Value::object();
            for (c, val) in values {
                obj.insert(c.id(), num4(*val));
            }
            Value::object().with("values", obj)
        }
        FillSource::Material { path, params } => {
            let p = params.iter().map(|(k, v)| (k.clone(), Value::from(v.as_str()))).collect();
            Value::object().with("material", path.as_str()).with("params", Value::Object(p))
        }
    };
    extras.inject(here, &mut v);
    v
}

/// Reads a layer list; `at` is its JSON path.
pub fn read_all(list: &[Value], at: &str, set: &SetScope, ctx: &mut ReadCtx) -> Result<Vec<Layer>, FormatError> {
    list.iter().enumerate().map(|(i, v)| read(v, format!("{at}[{i}]"), set, ctx)).collect()
}

fn read(v: &Value, at: String, set: &SetScope, ctx: &mut ReadCtx) -> Result<Layer, FormatError> {
    let mut o = Obj::new(v, at)?;
    let id = o.str("id")?.to_string();
    let here = loc(set.loc, "layers", &id);
    let mut layer = Layer::new(id.clone(), o.str("name")?, LayerKind::Group(Vec::new()));
    layer.visible = o.bool_or("visible", true)?;
    layer.opacity = o.f32_or("opacity", 1.0)?;
    layer.blend = blend::read_channels(&mut o, &here, ctx)?;
    if let Some(m) = o.opt("mask") {
        layer.mask = Some(effect_mask::read(m, o.path("mask"), set, &id, &format!("{here}/mask"), ctx)?);
    }
    if let Some(g) = o.opt("geometry_mask") {
        layer.geometry_mask = Some(geometry_mask::read(g, o.path("geometry_mask"), &format!("{here}/geometry_mask"), ctx)?);
    }
    let kind = o.str("kind")?;
    layer.kind = match kind {
        "paint" => {
            let mut content = PaintContent::default();
            for (key, r) in o.entries("images")? {
                let c = channel(key, &o.path("images"))?;
                let slot = ImageSlot::Layer { set: set.index, layer: id.clone(), image: LayerImage::Paint(c) };
                let rat = format!("{}.{key}", o.path("images"));
                let (img, cs) = image_ref::read(r, rat.clone(), &loc(&here, "images", key), slot, ctx)?;
                if cs != set.color_space(c) {
                    return Err(FormatError::manifest(rat, "colour space differs from the channel's"));
                }
                content.images.push((c, img));
            }
            LayerKind::Paint(content)
        }
        "fill" => {
            let f = read_fill(o.req("fill")?, o.path("fill"), &format!("{here}/fill"), ctx)?;
            LayerKind::Fill(f)
        }
        "group" => {
            let children = o.array("children")?;
            LayerKind::Group(read_all(children, &o.path("children"), set, ctx)?)
        }
        other => return Err(o.error("kind", format!("unknown layer kind {other:?}"))),
    };
    let effects = o.array("effects")?;
    layer.effects = effect::read_all(effects, &o.path("effects"), set, &id, &here, ctx)?;
    ctx.extras.record(&here, &o);
    Ok(layer)
}

/// Reads a fill object (`at` is its JSON path, `here` its extras location).
pub fn read_fill(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Fill, FormatError> {
    let mut o = Obj::new(v, at)?;
    let source = match o.opt_str("material")? {
        Some(path) => {
            let mut params = Vec::new();
            for (k, val) in o.entries("params")? {
                let val = val.as_str().ok_or_else(|| o.error("params", format!("{k:?}: expected a string")))?;
                params.push((k.clone(), val.to_string()));
            }
            FillSource::Material { path: path.to_string(), params }
        }
        None => {
            let mut values = Vec::new();
            for (k, val) in o.entries("values")? {
                let c = channel(k, &o.path("values"))?;
                let nums: Option<Vec<f32>> = val.as_array().filter(|a| a.len() == 4).and_then(|a| a.iter().map(as_f32).collect());
                let nums = nums.ok_or_else(|| o.error("values", format!("{k}: expected 4 numbers")))?;
                values.push((c, [nums[0], nums[1], nums[2], nums[3]]));
            }
            FillSource::Uniform(values)
        }
    };
    ctx.extras.record(here, &o);
    Ok(Fill { source })
}
