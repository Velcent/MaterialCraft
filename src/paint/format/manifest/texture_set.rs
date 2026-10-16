//! Texture sets: `{"id", "name", "mesh_material", "uv_set", "resolution", "channels", "layers",
//! "baked"}`.

use super::extras::loc;
use super::fields::Obj;
use super::layer::{self, SetScope};
use super::{image_ref, ReadCtx, WriteCtx};
use crate::json::Value;
use crate::paint::format::slot::ImageSlot;
use crate::paint::format::FormatError;
use crate::paint::{BakedKind, BakedMap, ChannelSpec, ColorSpace, PixelFormat, TextureSet};

pub fn write(index: usize, set: &TextureSet, ctx: &WriteCtx) -> Value {
    let here = loc("", "texture_sets", &set.id);
    let scope = SetScope {
        index,
        loc: &here,
        channels: &set.channels,
    };
    let channels: Value = set
        .channels
        .iter()
        .map(|c| {
            let mut v = Value::object()
                .with("id", c.channel.id())
                .with("format", c.format.id())
                .with("color_space", c.color_space.id());
            ctx.extras
                .inject(&loc(&here, "channels", &c.channel.id()), &mut v);
            v
        })
        .collect();
    let baked: Value = set
        .baked
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let bloc = loc(&here, "baked", &i.to_string());
            let path = ctx
                .plan
                .path(&ImageSlot::Baked {
                    set: index,
                    index: i,
                })
                .unwrap_or_default();
            let image = image_ref::write(
                path,
                &b.image,
                ColorSpace::Linear,
                &format!("{bloc}/image"),
                ctx.extras,
            );
            let mut v = Value::object()
                .with("kind", b.kind.id())
                .with("image", image);
            ctx.extras.inject(&bloc, &mut v);
            v
        })
        .collect();
    let mut v = Value::object()
        .with("id", set.id.as_str())
        .with("name", set.name.as_str())
        .with("mesh_material", set.material_slot)
        .with("uv_set", set.uv_set)
        .with("resolution", vec![set.resolution.0, set.resolution.1])
        .with("channels", channels)
        .with("layers", layer::write_all(&set.layers, &scope, ctx))
        .with("baked", baked);
    ctx.extras.inject(&here, &mut v);
    v
}

pub fn read(index: usize, v: &Value, ctx: &mut ReadCtx) -> Result<TextureSet, FormatError> {
    let mut o = Obj::new(v, format!("texture_sets[{index}]"))?;
    let id = o.str("id")?;
    let here = loc("", "texture_sets", id);
    let mut set = TextureSet::new(id, o.str("name")?, o.size("resolution")?);
    set.material_slot = o.usize("mesh_material")?;
    set.uv_set = o.usize("uv_set")?;
    for (i, c) in o.array("channels")?.iter().enumerate() {
        let mut co = Obj::new(c, format!("{}[{i}]", o.path("channels")))?;
        let cid = co.str("id")?;
        let channel = layer::channel(cid, co.at())?;
        let fid = co.str("format")?;
        let format = PixelFormat::from_id(fid)
            .ok_or_else(|| co.error("format", format!("unknown pixel format {fid:?}")))?;
        let cs = co.str("color_space")?;
        let color_space = ColorSpace::from_id(cs)
            .ok_or_else(|| co.error("color_space", format!("unknown colour space {cs:?}")))?;
        ctx.extras.record(&loc(&here, "channels", cid), &co);
        set.channels.push(ChannelSpec {
            channel,
            format,
            color_space,
        });
    }
    let scope = SetScope {
        index,
        loc: &here,
        channels: &set.channels,
    };
    let layers = layer::read_all(o.array("layers")?, &o.path("layers"), &scope, ctx)?;
    set.layers = layers;
    for (i, b) in o.array("baked")?.iter().enumerate() {
        let bloc = loc(&here, "baked", &i.to_string());
        let mut bo = Obj::new(b, format!("{}[{i}]", o.path("baked")))?;
        let kind = BakedKind::from_id(bo.str("kind")?);
        let slot = ImageSlot::Baked {
            set: index,
            index: i,
        };
        let (image, _) = image_ref::read(
            bo.req("image")?,
            bo.path("image"),
            &format!("{bloc}/image"),
            slot,
            ctx,
        )?;
        ctx.extras.record(&bloc, &bo);
        set.baked.push(BakedMap { kind, image });
    }
    ctx.extras.record(&here, &o);
    Ok(set)
}
