//! [`MetaEdit`]: a change to the user-facing metadata of an asset (name, tags, description), applied to the sidecar
//! as JSON so fields this version doesn't know (written by a newer MaterialCraft) are kept. [`SidecarText`] reads and
//! writes the sidecar's text.

use std::path::Path;

use super::StoreError;
use super::fs::StoreFs;
use crate::assets::{MAX_SIDECAR_BYTES, Sidecar};
use crate::json::{self, Value};

/// What to change; `None` leaves a field as it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetaEdit {
    /// The display name; an empty name removes it (the file name shows).
    pub name: Option<String>,
    /// The tags (trimmed, empty and repeated ones dropped).
    pub tags: Option<Vec<String>>,
    /// The description; `Some(None)` or an empty text removes it.
    pub description: Option<Option<String>>,
}

impl MetaEdit {
    /// Tags typed as one line, separated by commas.
    pub fn tags_from_text(text: &str) -> Vec<String> {
        clean_tags(text.split(',').map(str::to_owned))
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_none() && self.tags.is_none() && self.description.is_none()
    }

    /// Applies the change to a sidecar's JSON object.
    pub(crate) fn apply(&self, v: &mut Value) {
        if let Some(name) = &self.name {
            set_text(v, "name", Some(name));
        }
        if let Some(tags) = &self.tags {
            match clean_tags(tags.iter().cloned()) {
                t if t.is_empty() => {
                    v.remove("tags");
                }
                t => {
                    v.insert("tags", Value::Array(t.into_iter().map(Value::from).collect()));
                }
            }
        }
        if let Some(d) = &self.description {
            set_text(v, "description", d.as_deref());
        }
    }
}

fn set_text(v: &mut Value, key: &str, text: Option<&str>) {
    match text.map(str::trim).filter(|t| !t.is_empty()) {
        Some(t) => {
            v.insert(key, t);
        }
        None => {
            v.remove(key);
        }
    }
}

fn clean_tags(tags: impl Iterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim();
        if !t.is_empty() && !out.iter().any(|o| o == t) {
            out.push(t.to_owned());
        }
    }
    out
}

/// A sidecar's text: what the store writes, and the existing sidecar read for an edit.
pub(crate) struct SidecarText;

impl SidecarText {
    /// The text of a new sidecar (pretty JSON and a final newline).
    pub fn of(sidecar: &Sidecar) -> String {
        SidecarText::of_json(&sidecar.to_json())
    }

    pub fn of_json(v: &Value) -> String {
        format!("{}\n", v.to_string_pretty())
    }

    /// The sidecar at `path` as JSON, or a fresh one when there is none. A sidecar that doesn't read is an error:
    /// rewriting it could lose what the user wrote.
    pub fn read(fs: &dyn StoreFs, path: &Path, exists: bool) -> Result<Value, StoreError> {
        if !exists {
            return Ok(Sidecar::default().to_json());
        }
        let bad = |reason: String| StoreError::Sidecar { path: path.to_path_buf(), reason };
        let bytes = fs.read(path).map_err(|e| StoreError::io(path, e))?;
        if bytes.len() as u64 > MAX_SIDECAR_BYTES {
            return Err(bad("too large".into()));
        }
        let text = String::from_utf8(bytes).map_err(|_| bad("not UTF-8".into()))?;
        Sidecar::parse(&text).map_err(|e| bad(e.0))?;
        json::parse(&text).map_err(|e| bad(e.to_string()))
    }
}
