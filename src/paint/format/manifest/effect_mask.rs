//! Masks: `{"kind": "stack", "enabled", "initial", "effects"}` since 0.3, and [`DomainCodec`] for their
//! effects: `"blend"` is one `{"mode", "opacity"}`, paint pixels are one `"image"` (written `linear`), fill
//! values one number `"value"`.
//!
//! The 0.2 form `{"kind": "image", "image": …}` (value = red × alpha) is still read: it becomes a black mask
//! with the image painted on top ([`Mask::from_image`]), which is how it is written from then on.

use super::blend;
use super::effect::{self, DomainCodec, EffectAt};
use super::fields::{Obj, num};
use super::layer::SetScope;
use super::{ReadCtx, WriteCtx, image_ref};
use crate::json::Value;
use crate::paint::effect::Grey;
use crate::paint::format::FormatError;
use crate::paint::format::slot::ImageSlot;
use crate::paint::{ColorSpace, GreyBlend, LayerImage, Mask, MaskPixels};

fn slot(set: &SetScope, layer: &str, index: usize) -> ImageSlot {
    ImageSlot::Layer { set: set.index, layer: layer.to_string(), image: LayerImage::Mask { effect: index } }
}

impl DomainCodec for Grey {
    fn default_blend() -> GreyBlend {
        GreyBlend::default()
    }

    fn write_blend(b: &GreyBlend, at: &EffectAt, ctx: &WriteCtx) -> Value {
        blend::write_grey(b, &format!("{}/blend", at.here), ctx.extras)
    }

    fn read_blend(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<GreyBlend, FormatError> {
        blend::read_grey(o, &format!("{}/blend", at.here), ctx)
    }

    fn write_pixels(p: &MaskPixels, at: &EffectAt, ctx: &WriteCtx, v: &mut Value) {
        let path = ctx.plan.path(&slot(at.set, at.layer, at.index)).unwrap_or_default();
        v.insert("image", image_ref::write(path, &p.image, ColorSpace::Linear, &format!("{}/image", at.here), ctx.extras));
    }

    fn read_pixels(o: &mut Obj, at: &EffectAt, ctx: &mut ReadCtx) -> Result<MaskPixels, FormatError> {
        let (image, _) = image_ref::read(o.req("image")?, o.path("image"), &format!("{}/image", at.here), slot(at.set, at.layer, at.index), ctx)?;
        Ok(MaskPixels { image })
    }

    fn write_values(value: &f32, _: &EffectAt, _: &WriteCtx, v: &mut Value) {
        v.insert("value", num(*value));
    }

    fn read_values(o: &mut Obj, _: &EffectAt, _: &mut ReadCtx) -> Result<f32, FormatError> {
        o.f32_or("value", 1.0)
    }
}

/// The mask object; `here` is the extras location of the mask.
pub fn write(m: &Mask, set: &SetScope, layer: &str, here: &str, ctx: &WriteCtx) -> Value {
    let effects = effect::write_all(&m.effects, set, layer, here, ctx);
    let mut v = Value::object().with("kind", "stack").with("enabled", m.enabled).with("initial", num(m.initial)).with("effects", effects);
    ctx.extras.inject(here, &mut v);
    v
}

/// Reads a mask object (`at` is its JSON path, `here` its extras location).
pub fn read(v: &Value, at: String, set: &SetScope, layer: &str, here: &str, ctx: &mut ReadCtx) -> Result<Mask, FormatError> {
    let mut o = Obj::new(v, at)?;
    let mask = match o.str("kind")? {
        "stack" => {
            let list = o.array("effects")?;
            let effects = effect::read_all(list, &o.path("effects"), set, layer, here, ctx)?;
            Mask { enabled: o.bool_or("enabled", true)?, initial: o.f32_or("initial", 1.0)?, effects }
        }
        "image" => {
            // 0.2: the image becomes paint effect 0; its unknown fields move with it.
            let image_here = format!("{here}/effects/0/image");
            let (image, _) = image_ref::read(o.req("image")?, o.path("image"), &image_here, slot(set, layer, 0), ctx)?;
            Mask::from_image(image)
        }
        other => return Err(o.error("kind", format!("unknown mask kind {other:?}"))),
    };
    ctx.extras.record(here, &o);
    Ok(mask)
}
