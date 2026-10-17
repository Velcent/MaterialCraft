//! Geometry masks (since 0.3): `{"parts": ["<part name>", …]}` or `{"uv_tiles": [1001, …]}`.

use super::ReadCtx;
use super::fields::Obj;
use crate::json::Value;
use crate::paint::GeometryMask;
use crate::paint::format::FormatError;

pub fn write(g: &GeometryMask, here: &str, ctx: &super::WriteCtx) -> Value {
    let mut v = match g {
        GeometryMask::Parts(parts) => Value::object().with("parts", parts.iter().map(String::as_str).collect::<Value>()),
        GeometryMask::UvTiles(tiles) => Value::object().with("uv_tiles", tiles.clone()),
    };
    ctx.extras.inject(here, &mut v);
    v
}

pub fn read(v: &Value, at: String, here: &str, ctx: &mut ReadCtx) -> Result<GeometryMask, FormatError> {
    let mut o = Obj::new(v, at)?;
    let mask = if let Some(parts) = o.opt("parts") {
        let parts: Option<Vec<String>> = parts.as_array().and_then(|a| a.iter().map(|p| p.as_str().map(str::to_string)).collect());
        GeometryMask::Parts(parts.ok_or_else(|| o.error("parts", "expected an array of part names"))?)
    } else if let Some(tiles) = o.opt("uv_tiles") {
        let tiles: Option<Vec<u32>> = tiles.as_array().and_then(|a| a.iter().map(|t| t.as_u64().and_then(|t| u32::try_from(t).ok())).collect());
        GeometryMask::UvTiles(tiles.ok_or_else(|| o.error("uv_tiles", "expected an array of tile numbers"))?)
    } else {
        return Err(FormatError::manifest(o.at(), "expected \"parts\" or \"uv_tiles\""));
    };
    ctx.extras.record(here, &o);
    Ok(mask)
}
