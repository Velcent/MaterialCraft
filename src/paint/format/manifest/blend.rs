//! Blend entries: per channel `{"<channel>": {"mode", "opacity"}, …}` (layers and content effects) and the
//! single `{"mode", "opacity"}` of a mask effect.

use super::ReadCtx;
use super::extras::{Extras, loc};
use super::fields::{Obj, num};
use crate::json::Value;
use crate::paint::format::FormatError;
use crate::paint::{BlendMode, Channel, ChannelBlend, GreyBlend};

/// A channel → entry object; `here` is the extras location of the object holding it.
pub fn write_channels(entries: &[ChannelBlend], here: &str, extras: &Extras) -> Value {
    let mut blend = Value::object();
    for b in entries {
        let mut e = Value::object().with("mode", b.mode.id()).with("opacity", num(b.opacity));
        extras.inject(&loc(here, "blend", &b.channel.id()), &mut e);
        blend.insert(b.channel.id(), e);
    }
    blend
}

/// Reads the `blend` key of `o` (missing = no entries).
pub fn read_channels(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<Vec<ChannelBlend>, FormatError> {
    let mut out = Vec::new();
    for (key, b) in o.entries("blend")? {
        let channel = channel(key, &o.path("blend"))?;
        let mut e = Obj::new(b, format!("{}.{key}", o.path("blend")))?;
        let (mode, opacity) = mode_and_opacity(&mut e)?;
        ctx.extras.record(&loc(here, "blend", key), &e);
        out.push(ChannelBlend { channel, mode, opacity });
    }
    Ok(out)
}

/// A mask effect's `{"mode", "opacity"}`; `here` is the extras location of the blend object itself.
pub fn write_grey(b: &GreyBlend, here: &str, extras: &Extras) -> Value {
    let mut v = Value::object().with("mode", b.mode.id()).with("opacity", num(b.opacity));
    extras.inject(here, &mut v);
    v
}

/// Reads the `blend` key of `o` (missing = normal at full opacity).
pub fn read_grey(o: &mut Obj, here: &str, ctx: &mut ReadCtx) -> Result<GreyBlend, FormatError> {
    let Some(v) = o.opt("blend") else {
        return Ok(GreyBlend::default());
    };
    let mut e = Obj::new(v, o.path("blend"))?;
    let (mode, opacity) = mode_and_opacity(&mut e)?;
    ctx.extras.record(here, &e);
    Ok(GreyBlend { mode, opacity })
}

fn mode_and_opacity(e: &mut Obj) -> Result<(BlendMode, f32), FormatError> {
    let mode = e.str("mode")?;
    let mode = BlendMode::from_id(mode).ok_or_else(|| e.error("mode", format!("unknown blend mode {mode:?}")))?;
    Ok((mode, e.f32_or("opacity", 1.0)?))
}

/// A channel id; `at` names the object it is a key of.
pub fn channel(id: &str, at: &str) -> Result<Channel, FormatError> {
    Channel::from_id(id).ok_or_else(|| FormatError::manifest(at, format!("unknown channel {id:?}")))
}
