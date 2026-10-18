//! [`Writable`]: a library root the store may write to, and the checks that keep every path it writes inside it:
//! library-relative, `/`-separated, without empty, `.`, `..` or hidden segments, `\` or `:`, and (once resolved on
//! disk, following links) still under the root's folder.

use std::path::{Path, PathBuf};

use super::StoreError;
use crate::assets::format::FileRole;
use crate::assets::{Format, LibraryRoot};

/// A folder-backed, writable library root.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Writable<'a> {
    pub dir: &'a Path,
}

impl<'a> Writable<'a> {
    /// The root, if the store may write to it.
    pub fn of(root: &'a LibraryRoot) -> Result<Writable<'a>, StoreError> {
        if root.is_read_only() {
            return Err(StoreError::ReadOnly { library: root.name.clone() });
        }
        let dir = root.path.as_deref().ok_or_else(|| StoreError::NoFolder { library: root.name.clone() })?;
        Ok(Writable { dir })
    }

    /// The file of the asset at library-relative `rel`, which must name an asset file.
    pub fn asset(&self, rel: &str) -> Result<(PathBuf, Format), StoreError> {
        check_relative(rel, false)?;
        let FileRole::Asset(format) = FileRole::of(rel) else { return Err(StoreError::NotAnAsset(rel.to_owned())) };
        Ok((self.inside(rel)?, format))
    }

    /// The folder at library-relative `rel` (empty for the root itself).
    pub fn folder(&self, rel: &str) -> Result<PathBuf, StoreError> {
        check_relative(rel, true)?;
        self.inside(rel)
    }

    /// `rel` under the root's folder, refused when the nearest part of it that exists resolves outside the folder
    /// (a link pointing elsewhere).
    fn inside(&self, rel: &str) -> Result<PathBuf, StoreError> {
        let path = rel.split('/').filter(|s| !s.is_empty()).fold(self.dir.to_path_buf(), |p, s| p.join(s));
        let Ok(root) = self.dir.canonicalize() else { return Ok(path) };
        let existing = path.ancestors().find_map(|p| p.canonicalize().ok());
        match existing {
            Some(real) if real.starts_with(&root) => Ok(path),
            _ => Err(StoreError::bad_path(rel, "leads outside the library")),
        }
    }
}

/// Checks a library-relative path; `folder` allows the empty path (the root).
pub(crate) fn check_relative(rel: &str, folder: bool) -> Result<(), StoreError> {
    if rel.is_empty() {
        return if folder { Ok(()) } else { Err(StoreError::bad_path(rel, "is empty")) };
    }
    if rel.contains(['\\', ':', '\0']) {
        return Err(StoreError::bad_path(rel, "must be a library-relative path with / separators"));
    }
    for segment in rel.split('/') {
        match segment {
            "" => return Err(StoreError::bad_path(rel, "has an empty part (or starts with /)")),
            "." | ".." => return Err(StoreError::bad_path(rel, "must not contain . or ..")),
            s if s.starts_with('.') => return Err(StoreError::bad_path(rel, "must not name hidden files")),
            _ => {}
        }
    }
    Ok(())
}

/// `folder/file` (just `file` at the root).
pub(crate) fn join(folder: &str, file: &str) -> String {
    if folder.is_empty() { file.to_owned() } else { format!("{folder}/{file}") }
}
