//! Sidecar metadata: a small JSON file next to an asset (`<file>.mcmeta`) with its display name,
//! usage, tags, thumbnail and provenance. Every field is optional; a sidecar only overrides what the
//! catalogue would otherwise derive from the folder and file name.
//!
//! ```json
//! {"format": "materialcraft-asset", "version": 1, "name": "Brushed steel", "usage": "material",
//!  "tags": ["metal", "worn"], "thumbnail": "brushed-steel.png", "needs": ["curvature"],
//!  "provenance": {"kind": "graph", "graph": "graphs/brushed-steel.mca"}}
//! ```

use super::{Provenance, Usage};
use crate::json::{self, Value};

/// Value of the sidecar's `format` field.
pub const SIDECAR_FORMAT: &str = "materialcraft-asset";
/// Newest sidecar version this module writes and reads.
pub const SIDECAR_VERSION: u64 = 1;

/// The parsed contents of a sidecar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Sidecar {
    pub name: Option<String>,
    pub usage: Option<Usage>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    /// Thumbnail image, relative to the asset's folder.
    pub thumbnail: Option<String>,
    /// Baked maps a smart material or mask needs (AS-05), by map name.
    pub needs: Vec<String>,
    pub provenance: Provenance,
}

/// A sidecar that could not be read; the asset stays in the catalogue without it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarError(pub String);

impl Sidecar {
    /// Parse a sidecar. Unknown fields are ignored (newer writers may add some); a wrong type on a
    /// known field, a newer version or another format is an error.
    pub fn parse(text: &str) -> Result<Sidecar, SidecarError> {
        let v = json::parse(text).map_err(|e| SidecarError(format!("not valid JSON: {e}")))?;
        if v.as_object().is_none() {
            return Err(SidecarError("a sidecar must be a JSON object".into()));
        }
        match v.get("format") {
            None => {}
            Some(Value::String(f)) if f == SIDECAR_FORMAT => {}
            Some(other) => return Err(SidecarError(format!("format is {other}, expected \"{SIDECAR_FORMAT}\""))),
        }
        if let Some(ver) = v.get("version") {
            match ver.as_u64() {
                Some(n) if n <= SIDECAR_VERSION => {}
                Some(n) => return Err(SidecarError(format!("version {n} is newer than this MaterialCraft reads ({SIDECAR_VERSION})"))),
                None => return Err(SidecarError("version must be a whole number".into())),
            }
        }
        let usage = match opt_str(&v, "usage")? {
            None => None,
            Some(s) => Some(Usage::from_name(&s).ok_or_else(|| SidecarError(format!("unknown usage {s:?}")))?),
        };
        let provenance = Provenance::from_json(v.get("provenance").unwrap_or(&Value::Null)).map_err(SidecarError)?;
        Ok(Sidecar {
            name: opt_str(&v, "name")?,
            usage,
            tags: str_list(&v, "tags")?,
            description: opt_str(&v, "description")?,
            thumbnail: opt_str(&v, "thumbnail")?,
            needs: str_list(&v, "needs")?,
            provenance,
        })
    }

    /// The sidecar as JSON, with the format marker and version first.
    pub fn to_json(&self) -> Value {
        let mut v = Value::object().with("format", SIDECAR_FORMAT).with("version", SIDECAR_VERSION);
        if let Some(n) = &self.name {
            v.insert("name", n.as_str());
        }
        if let Some(u) = self.usage {
            v.insert("usage", u.keyword());
        }
        if !self.tags.is_empty() {
            v.insert("tags", Value::Array(self.tags.iter().map(|t| Value::from(t.as_str())).collect()));
        }
        if let Some(d) = &self.description {
            v.insert("description", d.as_str());
        }
        if let Some(t) = &self.thumbnail {
            v.insert("thumbnail", t.as_str());
        }
        if !self.needs.is_empty() {
            v.insert("needs", Value::Array(self.needs.iter().map(|t| Value::from(t.as_str())).collect()));
        }
        if let Some(p) = self.provenance.to_json() {
            v.insert("provenance", p);
        }
        v
    }
}

fn opt_str(v: &Value, key: &str) -> Result<Option<String>, SidecarError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(SidecarError(format!("{key} must be a string"))),
    }
}

fn str_list(v: &Value, key: &str) -> Result<Vec<String>, SidecarError> {
    match v.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => {
            items.iter().map(|i| i.as_str().map(str::to_owned).ok_or_else(|| SidecarError(format!("{key} must hold only strings")))).collect()
        }
        Some(_) => Err(SidecarError(format!("{key} must be a list of strings"))),
    }
}
