//! The standard library layout (AS-02): one folder per usage, created when a library is added.

use std::fs;
use std::io;
use std::path::PathBuf;

use super::{LibraryRoot, Usage};

/// Create the usage folders that don't exist yet under a writable, folder-backed root. Returns the
/// folders it created. The starter library is read-only and is refused.
pub fn create_standard_layout(root: &LibraryRoot) -> io::Result<Vec<PathBuf>> {
    if root.is_read_only() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, format!("library {} is read-only", root.name)));
    }
    let Some(dir) = &root.path else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("library {} has no folder", root.name)));
    };
    let mut created = Vec::new();
    for usage in Usage::ALL {
        let folder = dir.join(usage.folder());
        if !folder.is_dir() {
            fs::create_dir_all(&folder)?;
            created.push(folder);
        }
    }
    Ok(created)
}
