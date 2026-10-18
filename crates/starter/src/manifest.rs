//! `starter.json` at the library's root: marks the folder as a starter library, names the [`LIBRARY_VERSION`] it
//! was built at and lists its assets. The catalogue ignores it (it is not an asset format).

use crate::StarterError;
use materialcraft::assets::Usage;
use materialcraft::json::{self, Value};
use std::fs;
use std::path::Path;

/// The manifest's file name.
pub const MANIFEST: &str = "starter.json";

/// The starter library's content version. Bump it whenever a recipe changes what it makes.
/// Version 2: brush presets, and the procedural alphas name their tip in their provenance.
/// Version 3: seeded tip variants, PhotoCraft's patterns and Texturizer reliefs, presets translated from PhotoCraft's
/// and the physicallybased.info reference materials.
pub const LIBRARY_VERSION: u64 = 3;

const FORMAT: &str = "materialcraft-starter";

/// Writes the manifest listing `assets` into `root`.
pub(crate) fn write(root: &Path, assets: &[(String, Usage)]) -> Result<(), StarterError> {
    let list = Value::Array(assets.iter().map(|(path, usage)| Value::object().with("path", path.as_str()).with("usage", usage.keyword())).collect());
    let v = Value::object().with("format", FORMAT).with("version", LIBRARY_VERSION).with("assets", list);
    let path = root.join(MANIFEST);
    fs::write(&path, format!("{}\n", v.to_string_pretty())).map_err(|e| StarterError::io(path, e))
}

/// The version of the starter library in `dir`, if it holds one.
pub fn installed_version(dir: &Path) -> Option<u64> {
    let text = fs::read_to_string(dir.join(MANIFEST)).ok()?;
    let v = json::parse(&text).ok()?;
    (v.get("format")?.as_str()? == FORMAT).then(|| v.get("version")?.as_u64())?
}

/// True if `dir` holds a starter library of this build's [`LIBRARY_VERSION`].
pub fn is_current(dir: &Path) -> bool {
    installed_version(dir) == Some(LIBRARY_VERSION)
}

/// True if `dir` holds a starter library of any version.
pub(crate) fn is_starter(dir: &Path) -> bool {
    installed_version(dir).is_some()
}
