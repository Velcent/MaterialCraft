//! How an asset was made (ADR 0007 starter assets, ADR 0008 rm-starter-scope): every starter asset
//! records it, so the shipped library can be regenerated and its origin checked.

use crate::json::Value;

/// The origin of an asset, as recorded in its sidecar.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Provenance {
    /// Nothing recorded (the asset has no sidecar, or the sidecar says nothing).
    #[default]
    Unknown,
    /// Rendered from a MaterialCraft graph.
    Graph {
        /// The graph file or id it was rendered from.
        graph: String,
        /// Parameter values used, as the graph's own JSON (opaque here).
        parameters: Option<Value>,
    },
    /// Produced by a built-in generator with a preset.
    Generator {
        /// The generator id.
        generator: String,
        /// The preset's parameter values (opaque here).
        preset: Option<Value>,
    },
    /// Made by hand.
    HandMade { author: Option<String>, notes: Option<String> },
    /// Taken from a public CC0 source.
    Cc0 {
        /// Where it was downloaded.
        url: String,
        /// Licence identifier; always CC0 for shipped assets.
        licence: String,
        author: Option<String>,
    },
}

/// The licence id written when a CC0 source records none.
pub const CC0: &str = "CC0-1.0";

impl Provenance {
    /// Stable keyword of the kind, used in sidecars and by the `source:` query field.
    pub fn kind(&self) -> &'static str {
        match self {
            Provenance::Unknown => "unknown",
            Provenance::Graph { .. } => "graph",
            Provenance::Generator { .. } => "generator",
            Provenance::HandMade { .. } => "hand-made",
            Provenance::Cc0 { .. } => "cc0",
        }
    }

    /// Read the sidecar's `provenance` object. Errors describe what is wrong, for the Assets log.
    pub fn from_json(v: &Value) -> Result<Provenance, String> {
        if v.is_null() {
            return Ok(Provenance::Unknown);
        }
        if v.as_object().is_none() {
            return Err("provenance must be an object".into());
        }
        let text = |key: &str| -> Result<Option<String>, String> {
            match v.get(key) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => Ok(Some(s.clone())),
                Some(_) => Err(format!("provenance.{key} must be a string")),
            }
        };
        let required = |key: &str| -> Result<String, String> { text(key)?.ok_or_else(|| format!("provenance.{key} is missing")) };
        let kind = required("kind")?;
        match kind.as_str() {
            "unknown" => Ok(Provenance::Unknown),
            "graph" => Ok(Provenance::Graph { graph: required("graph")?, parameters: v.get("parameters").cloned() }),
            "generator" => Ok(Provenance::Generator { generator: required("generator")?, preset: v.get("preset").cloned() }),
            "hand-made" => Ok(Provenance::HandMade { author: text("author")?, notes: text("notes")? }),
            "cc0" => {
                let licence = text("licence")?.unwrap_or_else(|| CC0.to_owned());
                if !licence.eq_ignore_ascii_case(CC0) && !licence.eq_ignore_ascii_case("CC0") {
                    return Err(format!("provenance.licence is {licence:?}; only CC0 sources are allowed"));
                }
                Ok(Provenance::Cc0 { url: required("url")?, licence, author: text("author")? })
            }
            other => Err(format!("unknown provenance kind {other:?}")),
        }
    }

    /// The sidecar's `provenance` object, or `None` for [`Provenance::Unknown`].
    pub fn to_json(&self) -> Option<Value> {
        let base = Value::object().with("kind", self.kind());
        let opt = |v: Value, key: &str, s: &Option<String>| match s {
            Some(s) => v.with(key, s.as_str()),
            None => v,
        };
        let raw = |v: Value, key: &str, x: &Option<Value>| match x {
            Some(x) => v.with(key, x.clone()),
            None => v,
        };
        match self {
            Provenance::Unknown => None,
            Provenance::Graph { graph, parameters } => Some(raw(base.with("graph", graph.as_str()), "parameters", parameters)),
            Provenance::Generator { generator, preset } => Some(raw(base.with("generator", generator.as_str()), "preset", preset)),
            Provenance::HandMade { author, notes } => Some(opt(opt(base, "author", author), "notes", notes)),
            Provenance::Cc0 { url, licence, author } => Some(opt(base.with("url", url.as_str()).with("licence", licence.as_str()), "author", author)),
        }
    }
}
