//! `"strokes": <entry path>` on paint layers and paint effects (since 0.4): where the stroke record of the
//! pixels is stored. Absent when there is none.

use super::fields::Obj;
use super::{ReadCtx, WriteCtx};
use crate::json::Value;
use crate::paint::format::FormatError;
use crate::paint::format::strokes::{StrokeRef, StrokeSlot};

/// Adds the field when `slot` has a stored record.
pub fn write(slot: &StrokeSlot, ctx: &WriteCtx, v: &mut Value) {
    if let Some(path) = ctx.strokes.path(slot) {
        v.insert("strokes", path);
    }
}

/// Reads the field and registers the record to load into `slot`.
pub fn read(o: &mut Obj, slot: StrokeSlot, ctx: &mut ReadCtx) -> Result<(), FormatError> {
    if let Some(path) = o.opt_str("strokes")? {
        ctx.strokes.push(StrokeRef { slot, path: path.to_string() });
    }
    Ok(())
}
