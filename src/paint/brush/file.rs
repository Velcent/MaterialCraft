//! A brush preset as a library asset (AS-06): `brushes/<name>.mcbrush`, the preset's JSON with a format tag. An
//! alpha-image tip is referenced by its library path, never copied in; a procedural tip needs no image.
//!
//! Writing and reading back gives the same preset bit for bit: numbers are written as the shortest text that parses
//! back to the same `f32`.

use super::BrushPreset;
use crate::json::{self, Value};
use std::fmt;

/// The file extension of a brush preset asset.
pub const EXTENSION: &str = "mcbrush";

/// The format tag in every preset file.
pub const FORMAT: &str = "materialcraft-brush";

/// Why a preset file can't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetFileError(pub String);

impl fmt::Display for PresetFileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PresetFileError {}

impl BrushPreset {
    /// The preset as the text of a `.mcbrush` file (pretty JSON, ending in a newline).
    pub fn to_file_text(&self) -> String {
        let v = self.to_json().with("format", FORMAT);
        format!("{}\n", v.to_string_pretty())
    }

    /// Reads a `.mcbrush` file's text. A file without the format tag is read as long as it is a JSON object (presets
    /// written by hand).
    pub fn from_file_text(text: &str) -> Result<BrushPreset, PresetFileError> {
        let v = json::parse(text).map_err(|e| PresetFileError(format!("not JSON: {e}")))?;
        match v.get("format").and_then(Value::as_str) {
            Some(f) if f != FORMAT => return Err(PresetFileError(format!("a {f:?} file, not a brush preset"))),
            _ => {}
        }
        BrushPreset::from_json(&v).ok_or_else(|| PresetFileError("not a brush preset (no JSON object)".into()))
    }
}
