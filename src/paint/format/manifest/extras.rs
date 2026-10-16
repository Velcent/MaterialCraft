//! [`Extras`]: manifest fields this version does not know (written by a newer minor version),
//! kept across load → save.
//!
//! The document model has no place for them, so they travel as the foreign file
//! [`Extras::PATH`]: a JSON object mapping a location (`""` for the root, `texture_sets/<id>`,
//! `texture_sets/<id>/layers/<id>/mask`, …) to the unknown fields of the object there. On load
//! the file is rebuilt from the manifest; on save its fields are written back into the matching
//! objects (fields we write ourselves take precedence). Objects that no longer exist drop theirs.

use super::fields::Obj;
use crate::json::{self, Value};
use crate::paint::format::FormatError;
use crate::paint::{Document, ForeignFile};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extras {
    entries: Vec<(String, Vec<(String, Value)>)>,
}

impl Extras {
    /// Foreign file path holding the unknown fields of a loaded document.
    pub const PATH: &'static str = "foreign/mcpaint/unknown-fields.json";

    /// The extras stored in `doc` (none if it has no [`Self::PATH`] file).
    pub fn of_document(doc: &Document) -> Result<Extras, FormatError> {
        let Some(file) = doc.foreign_file(Self::PATH) else {
            return Ok(Extras::default());
        };
        let bad = |m: &str| FormatError::Unsupported(format!("{}: {m}", Self::PATH));
        let text = std::str::from_utf8(&file.bytes).map_err(|_| bad("not UTF-8"))?;
        let root = json::parse(text).map_err(|e| bad(&e.to_string()))?;
        let entries = root.as_object().ok_or_else(|| bad("expected an object"))?;
        let entries = entries
            .iter()
            .map(|(loc, fields)| {
                let fields = fields.as_object().ok_or_else(|| bad("expected objects"))?;
                Ok((loc.clone(), fields.clone()))
            })
            .collect::<Result<_, FormatError>>()?;
        Ok(Extras { entries })
    }

    /// Remembers the keys of `obj` that were not read, under `location`.
    pub fn record(&mut self, location: &str, obj: &Obj) {
        let unread = obj.unread();
        if !unread.is_empty() {
            self.entries.push((location.to_string(), unread));
        }
    }

    /// Adds the fields stored for `location` to `target`, skipping keys it already has.
    pub fn inject(&self, location: &str, target: &mut Value) {
        let Some(fields) = self
            .entries
            .iter()
            .find(|(l, _)| l == location)
            .map(|e| &e.1)
        else {
            return;
        };
        for (k, v) in fields {
            if target.get(k).is_none() {
                target.insert(k.clone(), v.clone());
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The foreign file carrying these extras, or `None` if there are none.
    pub fn to_foreign(&self) -> Option<ForeignFile> {
        if self.entries.is_empty() {
            return None;
        }
        let root = Value::Object(
            self.entries
                .iter()
                .map(|(l, f)| (l.clone(), Value::Object(f.clone())))
                .collect(),
        );
        Some(ForeignFile {
            path: Self::PATH.to_string(),
            bytes: root.to_string_pretty().into_bytes(),
        })
    }
}

/// Location strings: ids are escaped like JSON pointer tokens (`~` → `~0`, `/` → `~1`).
pub fn loc(parent: &str, kind: &str, id: &str) -> String {
    let id = id.replace('~', "~0").replace('/', "~1");
    if parent.is_empty() {
        format!("{kind}/{id}")
    } else {
        format!("{parent}/{kind}/{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_inject() {
        let v = json::parse(r#"{"id": "a", "future": [1, 2], "id2": true}"#).unwrap();
        let mut o = Obj::new(&v, "x").unwrap();
        o.opt("id");
        let mut e = Extras::default();
        e.record("texture_sets/a", &o);
        let file = e.to_foreign().unwrap();
        let doc = Document {
            foreign: vec![file],
            ..Document::default()
        };
        let e2 = Extras::of_document(&doc).unwrap();
        assert_eq!(e2, e);
        let mut target = Value::object().with("id2", false);
        e2.inject("texture_sets/a", &mut target);
        assert_eq!(target.to_string(), r#"{"id2":false,"future":[1,2]}"#);
        assert_eq!(loc("", "texture_sets", "a/b~"), "texture_sets/a~1b~0");
    }
}
