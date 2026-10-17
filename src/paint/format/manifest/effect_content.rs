//! [`DomainCodec`] for content effects: `"blend"` per channel as on layers, paint pixels as `"images"`
//! (channel → image reference, in the channel's colour space) and `"strokes"`, fill values as `"fill"` (the
//! object of a fill layer).

use super::blend;
use super::effect::{DomainCodec, EffectAt};
use super::extras::loc;
use super::fields::Obj;
use super::layer::{read_fill, write_fill};
use super::{ReadCtx, WriteCtx, image_ref, strokes};
use crate::json::Value;
use crate::paint::effect::Content;
use crate::paint::format::FormatError;
use crate::paint::format::slot::ImageSlot;
use crate::paint::format::strokes::StrokeSlot;
use crate::paint::stroke::StrokeOwner;
use crate::paint::{ChannelBlend, Fill, LayerImage, PaintContent};

fn slot(at: &EffectAt, channel: crate::paint::Channel) -> ImageSlot {
    ImageSlot::Layer { set: at.set.index, layer: at.layer.to_string(), image: LayerImage::Effect { effect: at.index, channel } }
}

fn stroke_slot(at: &EffectAt) -> StrokeSlot {
    StrokeSlot { set: at.set.index, layer: at.layer.to_string(), owner: StrokeOwner::Effect { effect: at.index } }
}

impl DomainCodec for Content {
    fn default_blend() -> Vec<ChannelBlend> {
        Vec::new()
    }

    fn write_blend(entries: &Vec<ChannelBlend>, at: &EffectAt, ctx: &WriteCtx) -> Value {
        blend::write_channels(entries, &at.here, ctx.extras)
    }

    fn read_blend(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<Vec<ChannelBlend>, FormatError> {
        blend::read_channels(o, &at.here, ctx)
    }

    fn write_pixels(p: &PaintContent, at: &EffectAt, ctx: &WriteCtx, v: &mut Value) {
        let mut images = Value::object();
        for (c, img) in &p.images {
            let path = ctx.plan.path(&slot(at, *c)).unwrap_or_default();
            images.insert(c.id(), image_ref::write(path, img, at.set.color_space(*c), &loc(&at.here, "images", &c.id()), ctx.extras));
        }
        v.insert("images", images);
        strokes::write(&stroke_slot(at), ctx, v);
    }

    fn read_pixels(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<PaintContent, FormatError> {
        let mut content = PaintContent::default();
        for (key, r) in o.entries("images")? {
            let c = blend::channel(key, &o.path("images"))?;
            let rat = format!("{}.{key}", o.path("images"));
            let (img, cs) = image_ref::read(r, rat.clone(), &loc(&at.here, "images", key), slot(at, c), ctx)?;
            if cs != at.set.color_space(c) {
                return Err(FormatError::manifest(rat, "colour space differs from the channel's"));
            }
            content.images.push((c, img));
        }
        strokes::read(o, stroke_slot(at), ctx)?;
        Ok(content)
    }

    fn write_values(f: &Fill, at: &EffectAt, ctx: &WriteCtx, v: &mut Value) {
        v.insert("fill", write_fill(f, &format!("{}/fill", at.here), ctx.extras));
    }

    fn read_values(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<Fill, FormatError> {
        match o.opt("fill") {
            Some(f) => read_fill(f, o.path("fill"), &format!("{}/fill", at.here), ctx),
            None => Ok(Fill::uniform(Vec::new())),
        }
    }
}
