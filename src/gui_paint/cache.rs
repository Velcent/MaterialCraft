//! Where converted `.spp` projects are kept: `<OS temp>/materialcraft-paint/<name>-<key>.mcpaint`.
//! The key covers the source path, size and modification time and the importer build, so a changed
//! project or a new importer converts again while reopening an unchanged one is instant.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The folder holding converted documents.
pub fn dir() -> PathBuf {
    std::env::temp_dir().join("materialcraft-paint")
}

/// The cache file for `source` converted by `importer`.
pub fn converted_path(source: &Path, importer: &Path) -> PathBuf {
    let mut h = DefaultHasher::new();
    for p in [source, importer] {
        p.hash(&mut h);
        if let Ok(m) = std::fs::metadata(p) {
            m.len().hash(&mut h);
            m.modified().ok().hash(&mut h);
        }
    }
    let stem: String = source
        .file_stem()
        .map(|s| {
            s.to_string_lossy()
                .chars()
                .filter(|c| c.is_alphanumeric() || "-_".contains(*c))
                .collect()
        })
        .unwrap_or_default();
    dir().join(format!("{stem}-{:016x}.mcpaint", h.finish()))
}
