//! Stroke record entries (since 0.4): one binary entry per record next to the paint pixels it belongs to,
//! referenced from the manifest by `"strokes": <path>`.
//!
//! - [`codec`]: the entry's bytes.
//! - [`StrokePlan`]: entry names when writing; [`read_entry`]: an entry as [`StrokeData`] when reading. An
//!   entry that is missing, damaged or of a newer version does not stop the load: it becomes
//!   [`StrokeData::Unreadable`] (bytes kept when there are any) and [`crate::paint::stroke::problems`] lists it.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod codec;
#[cfg(test)]
pub(crate) mod tests;

use super::paths::ImagePlan;
use crate::paint::Document;
use crate::paint::stroke::{StrokeData, StrokeOwner, UnreadableStrokes};
use crate::zip::ZipReader;
use std::collections::HashMap;
use std::io::{Read, Seek};

/// Identifies one stroke record of a document. Layers are found by id (unique within their set).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StrokeSlot {
    pub set: usize,
    pub layer: String,
    pub owner: StrokeOwner,
}

impl StrokeSlot {
    /// The record slot this names in `doc`, if the layer has paint pixels there.
    pub fn strokes_mut<'d>(&self, doc: &'d mut Document) -> Option<&'d mut Option<StrokeData>> {
        doc.texture_sets.get_mut(self.set)?.layer_mut(&self.layer)?.strokes_mut(self.owner)
    }
}

/// A record the manifest refers to, to be loaded into `slot`.
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeRef {
    pub slot: StrokeSlot,
    pub path: String,
}

/// The entry name of every stroke record that is written, in writing order: `strokes.mcstroke`,
/// `effect-<index>-strokes.mcstroke` and `mask-<index>-strokes.mcstroke` in the layer's image directory.
/// Unreadable records without bytes have no entry.
pub struct StrokePlan<'d> {
    entries: Vec<(String, &'d StrokeData)>,
    index: HashMap<StrokeSlot, usize>,
}

impl<'d> StrokePlan<'d> {
    pub fn new(doc: &'d Document, images: &ImagePlan) -> StrokePlan<'d> {
        let mut plan = StrokePlan { entries: Vec::new(), index: HashMap::new() };
        for (s, set) in doc.texture_sets.iter().enumerate() {
            for (_, layer) in set.walk_layers() {
                let Some(dir) = images.layer_dir(s, &layer.id) else { continue };
                for (owner, data) in layer.stroke_records() {
                    if matches!(data, StrokeData::Unreadable(u) if u.bytes.is_empty()) {
                        continue;
                    }
                    let stem = match owner {
                        StrokeOwner::Paint => "strokes".to_string(),
                        StrokeOwner::Effect { effect } => format!("effect-{effect}-strokes"),
                        StrokeOwner::Mask { effect } => format!("mask-{effect}-strokes"),
                    };
                    plan.index.insert(StrokeSlot { set: s, layer: layer.id.clone(), owner }, plan.entries.len());
                    plan.entries.push((format!("{dir}/{stem}.{}", codec::EXTENSION), data));
                }
            }
        }
        plan
    }

    /// The entry name of `slot`, if it is written.
    pub fn path(&self, slot: &StrokeSlot) -> Option<&str> {
        self.index.get(slot).and_then(|&i| self.entries.get(i)).map(|(p, _)| p.as_str())
    }

    /// `(entry name, record)` in writing order.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &'d StrokeData)> + '_ {
        self.entries.iter().map(|(p, d)| (p.as_str(), *d))
    }
}

/// The bytes written for a record: its encoding, or the kept bytes of an unreadable one.
pub fn entry_bytes(data: &StrokeData) -> Result<Vec<u8>, String> {
    match data {
        StrokeData::Record(r) => codec::encode(r),
        StrokeData::Unreadable(u) => Ok(u.bytes.clone()),
    }
}

/// Reads the entry at `path`. Never fails: whatever cannot be read becomes [`StrokeData::Unreadable`].
pub fn read_entry<R: Read + Seek>(zip: &mut ZipReader<R>, path: &str) -> StrokeData {
    let unreadable = |bytes: Vec<u8>, reason: String| StrokeData::Unreadable(UnreadableStrokes { bytes, reason });
    if zip.find(path).is_none() {
        return unreadable(Vec::new(), format!("entry {path} is missing"));
    }
    match zip.read(path) {
        Ok(bytes) => match codec::decode(&bytes) {
            Ok(record) => StrokeData::Record(record),
            Err(reason) => unreadable(bytes, reason),
        },
        Err(e) => unreadable(Vec::new(), format!("entry {path}: {e}")),
    }
}
