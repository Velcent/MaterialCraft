//! [`RecordedBrush`]: what a stroke record needs besides its samples to replay one of our strokes: the preset, the
//! values painted per channel (or into a mask) and the viewport the stroke was painted on.
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
}

impl RecordedBrush {
    /// The value for `channel`, if the stroke painted it.
    pub fn value(&self, channel: Channel) -> Option<[f32; 4]> {
        self.values.iter().find(|(c, _)| *c == channel).map(|(_, v)| *v)
    }

    /// The reference to store in [`crate::paint::stroke::Stroke::preset`].
    pub fn to_reference(&self) -> String {
        let values = self.values.iter().fold(Value::object(), |o, (c, v)| o.with(c.id(), v.to_vec()));
        let v = Value::object().with("preset", self.preset.to_json()).with("values", values).with("mask", self.mask).with("viewport", self.viewport.to_vec());
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
        Some(RecordedBrush { preset, values, mask: field(&v, "mask").as_f64().unwrap_or(1.0) as f32, viewport })
    }
}
