//! Layers: `{"id", "name", "kind", "visible", "opacity", "blend", "mask", …}` plus `images`
//! (paint), `fill` (fill) or `children` (group).

use super::extras::{Extras, loc};
use super::fields::{Obj, as_f32, num, num4};
use super::{ReadCtx, WriteCtx, image_ref};
use crate::json::Value;
use crate::paint::format::FormatError;
use crate::paint::format::slot::ImageSlot;
use crate::paint::{BlendMode, Channel, ChannelBlend, ChannelSpec, ColorSpace, Fill, FillSource, Layer, LayerKind, Mask, PaintContent};

/// The texture set a layer belongs to, while writing or reading.
pub struct SetScope<'a> {
    pub index: usize,
    pub loc: &'a str,
    pub channels: &'a [ChannelSpec],
}

impl SetScope<'_> {
    fn color_space(&self, c: Channel) -> ColorSpace {
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
    let mut blend = Value::object();
    for b in &l.blend {
        let mut e = Value::object().with("mode", b.mode.id()).with("opacity", num(b.opacity));
        ctx.extras.inject(&loc(&here, "blend", &b.channel.id()), &mut e);
        blend.insert(b.channel.id(), e);
    }
    let mut v = Value::object()
        .with("id", l.id.as_str())
        .with("name", l.name.as_str())
        .with("kind", kind)
        .with("visible", l.visible)
        .with("opacity", num(l.opacity))
        .with("blend", blend);
    if let Some(Mask::Image(img)) = &l.mask {
        let slot = ImageSlot::Mask { set: set.index, layer: l.id.clone() };
        let path = ctx.plan.path(&slot).unwrap_or_default();
        let mloc = format!("{here}/mask");
        let mut m = Value::object().with("kind", "image");
        m.insert("image", image_ref::write(path, img, ColorSpace::Linear, &format!("{mloc}/image"), ctx.extras));
        ctx.extras.inject(&mloc, &mut m);
        v.insert("mask", m);
    }
    match &l.kind {
        LayerKind::Paint(p) => {
            let mut images = Value::object();
            for (c, img) in &p.images {
                let slot = ImageSlot::Paint { set: set.index, layer: l.id.clone(), channel: *c };
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
    ctx.extras.inject(&here, &mut v);
    v
}

fn write_fill(f: &Fill, here: &str, extras: &Extras) -> Value {
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
    for (key, b) in o.entries("blend")? {
        let channel = channel(key, &o.path("blend"))?;
        let mut e = Obj::new(b, format!("{}.{key}", o.path("blend")))?;
        let mode = e.str("mode")?;
        let mode = BlendMode::from_id(mode).ok_or_else(|| e.error("mode", format!("unknown blend mode {mode:?}")))?;
        let opacity = e.f32_or("opacity", 1.0)?;
        ctx.extras.record(&loc(&here, "blend", key), &e);
        layer.blend.push(ChannelBlend { channel, mode, opacity });
    }
    if let Some(m) = o.opt("mask") {
        let mloc = format!("{here}/mask");
        let mut mo = Obj::new(m, o.path("mask"))?;
        let kind = mo.str("kind")?;
        if kind != "image" {
            return Err(mo.error("kind", format!("unknown mask kind {kind:?}")));
        }
        let slot = ImageSlot::Mask { set: set.index, layer: id.clone() };
        let (img, _) = image_ref::read(mo.req("image")?, mo.path("image"), &format!("{mloc}/image"), slot, ctx)?;
        ctx.extras.record(&mloc, &mo);
        layer.mask = Some(Mask::Image(img));
    }
    let kind = o.str("kind")?;
    layer.kind = match kind {
        "paint" => {
            let mut content = PaintContent::default();
            for (key, r) in o.entries("images")? {
                let c = channel(key, &o.path("images"))?;
                let slot = ImageSlot::Paint { set: set.index, layer: id.clone(), channel: c };
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
    ctx.extras.record(&here, &o);
    Ok(layer)
}

fn read_fill(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<Fill, FormatError> {
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

/// A channel id; `at` names the object it is a key of.
pub fn channel(id: &str, at: &str) -> Result<Channel, FormatError> {
    Channel::from_id(id).ok_or_else(|| FormatError::manifest(at, format!("unknown channel {id:?}")))
}
