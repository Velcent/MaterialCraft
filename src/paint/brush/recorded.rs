//! [`RecordedBrush`]: what a stroke record needs besides its samples to replay one of our strokes: the preset, the
//! values painted per channel (or into a mask), the viewport the stroke was painted on and, for the retouch tools,
//! their settings ([`Retouch`]).
//!
//! It travels in the stroke's `preset` field as a reference that starts with [`REFERENCE_PREFIX`] followed by
//! compact JSON, so the record format stays as it is and a record that mixes imported strokes with ours says, per
//! stroke, how to reproduce it.

use super::json::field;
use super::preset::BrushPreset;
use crate::json::{self, Value};
use crate::paint::Channel;

/// Stroke-record preset references of strokes painted in MaterialCraft start with this.
pub const REFERENCE_PREFIX: &str = "materialcraft:brush:";

/// The retouch tools' settings of a stroke (smudge, blur, clone).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retouch {
    /// Smudge: how much of the carried paint each dab lays down and keeps; blur: how far each dab softens. 0 … 1.
    pub strength: f32,
    /// Clone: texel `(x, y)` takes the paint at `(x + clone_offset[0], y + clone_offset[1])`.
    pub clone_offset: [i32; 2],
}

impl Default for Retouch {
    fn default() -> Retouch {
        Retouch { strength: 0.5, clone_offset: [0, 0] }
    }
}

/// A stroke's brush as recorded with it.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedBrush {
    pub preset: BrushPreset,
    /// The value painted into each channel of a paint layer (stored encoding, straight RGBA).
    pub values: Vec<(Channel, [f32; 4])>,
    /// The grey value painted into a mask (0 black … 1 white).
    pub mask: f32,
    /// Size of the view the stroke was painted on (screen units); screen-sized brushes and the stored camera's
    /// samples are replayed on a view of this size.
    pub viewport: [f32; 2],
    /// Smudge, blur and clone settings (written only when not the defaults).
    pub retouch: Retouch,
}

impl RecordedBrush {
    /// The value for `channel`, if the stroke painted it.
    pub fn value(&self, channel: Channel) -> Option<[f32; 4]> {
        self.values.iter().find(|(c, _)| *c == channel).map(|(_, v)| *v)
    }

    /// The reference to store in [`crate::paint::stroke::Stroke::preset`].
    pub fn to_reference(&self) -> String {
        let values = self.values.iter().fold(Value::object(), |o, (c, v)| o.with(c.id(), v.to_vec()));
        let mut v =
            Value::object().with("preset", self.preset.to_json()).with("values", values).with("mask", self.mask).with("viewport", self.viewport.to_vec());
        if self.retouch != Retouch::default() {
            let r = Value::object().with("strength", self.retouch.strength).with("clone_offset", self.retouch.clone_offset.to_vec());
            v = v.with("retouch", r);
        }
        format!("{REFERENCE_PREFIX}{v}")
    }

    /// Reads a reference written by [`Self::to_reference`]; `None` for other references (imported strokes) and for
    /// damaged ones.
    pub fn from_reference(reference: &str) -> Option<RecordedBrush> {
        let v = json::parse(reference.strip_prefix(REFERENCE_PREFIX)?).ok()?;
        let preset = BrushPreset::from_json(field(&v, "preset"))?;
        let four = |v: &Value| -> Option<[f32; 4]> {
            let a = v.as_array()?;
            let n = |i: usize| a.get(i).and_then(Value::as_f64).map(|x| x as f32);
            Some([n(0)?, n(1)?, n(2)?, n(3)?])
        };
        let values = field(&v, "values").as_object()?.iter().filter_map(|(k, val)| Some((Channel::from_id(k)?, four(val)?))).collect();
        let vp = field(&v, "viewport");
        let viewport = [vp.at(0).unwrap_or(&Value::Null).as_f64()? as f32, vp.at(1).unwrap_or(&Value::Null).as_f64()? as f32];
        let retouch = match field(&v, "retouch") {
            Value::Null => Retouch::default(),
            r => {
                let o = field(r, "clone_offset");
                let i = |k: usize| o.at(k).and_then(Value::as_i64).and_then(|n| i32::try_from(n).ok());
                Retouch { strength: field(r, "strength").as_f64()? as f32, clone_offset: [i(0)?, i(1)?] }
            }
        };
        Some(RecordedBrush { preset, values, mask: field(&v, "mask").as_f64().unwrap_or(1.0) as f32, viewport, retouch })
    }
}
