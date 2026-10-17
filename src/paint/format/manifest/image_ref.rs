//! Image references: `{"path", "format", "color_space", "size"}`.

use super::extras::Extras;
use super::fields::Obj;
use super::{ImageRef, ReadCtx};
use crate::json::Value;
use crate::paint::format::FormatError;
use crate::paint::format::slot::ImageSlot;
use crate::paint::{ColorSpace, PixelFormat, TiledImage};

/// The reference to the image stored at `path`.
pub fn write(path: &str, img: &TiledImage, color_space: ColorSpace, loc: &str, extras: &Extras) -> Value {
    let (w, h) = img.size();
    let mut v = Value::object().with("path", path).with("format", img.format().id()).with("color_space", color_space.id()).with("size", vec![w, h]);
    extras.inject(loc, &mut v);
    v
}

/// Reads a reference, registers the image to load for `slot` and returns an empty image of the
/// declared size and format (filled in later) with its colour space.
pub fn read(v: &Value, at: String, loc: &str, slot: ImageSlot, ctx: &mut ReadCtx) -> Result<(TiledImage, ColorSpace), FormatError> {
    let mut o = Obj::new(v, at)?;
    let path = o.str("path")?.to_string();
    let format_id = o.str("format")?;
    let format = PixelFormat::from_id(format_id).ok_or_else(|| o.error("format", format!("unknown pixel format {format_id:?}")))?;
    let cs = o.str("color_space")?;
    let color_space = ColorSpace::from_id(cs).ok_or_else(|| o.error("color_space", format!("unknown colour space {cs:?}")))?;
    let size = o.size("size")?;
    let img = TiledImage::new(size.0, size.1, format).map_err(|e| o.error("size", e.to_string()))?;
    ctx.extras.record(loc, &o);
    ctx.images.push(ImageRef { slot, path, format, size });
    Ok((img, color_space))
}
