//! [`Bundle`]: the files that make up one asset on disk: the asset file, its sidecar (`<file>.mcmeta`) and its
//! conventional thumbnail (`<file>.thumb.png`). The store always moves or removes them together.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::fs::StoreFs;
use crate::assets::{SIDECAR_SUFFIX, THUMBNAIL_SUFFIX};

/// One asset's files, by absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bundle {
    pub file: PathBuf,
    pub sidecar: PathBuf,
    pub thumbnail: PathBuf,
}

/// Which of a bundle's companion files exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Present {
    pub sidecar: bool,
    pub thumbnail: bool,
}

impl Bundle {
    /// The files of the asset at `file` (whether they exist or not).
    pub fn of(file: &Path) -> Bundle {
        Bundle { file: file.to_path_buf(), sidecar: suffixed(file, SIDECAR_SUFFIX), thumbnail: suffixed(file, THUMBNAIL_SUFFIX) }
    }

    /// Which companions exist.
    pub fn present(&self, fs: &dyn StoreFs) -> Present {
        Present { sidecar: fs.exists(&self.sidecar), thumbnail: fs.exists(&self.thumbnail) }
    }

    /// The first of the three files that exists, if any (a new bundle must not replace anything).
    pub fn any_existing(&self, fs: &dyn StoreFs) -> Option<&Path> {
        [&self.file, &self.sidecar, &self.thumbnail].into_iter().find(|p| fs.exists(p)).map(PathBuf::as_path)
    }
}

/// `path` with `suffix` appended to its file name.
fn suffixed(path: &Path, suffix: &str) -> PathBuf {
    let mut s = OsString::from(path.as_os_str());
    s.push(suffix);
    PathBuf::from(s)
}

/// The root-relative paths of the asset at `rel`: the file, its sidecar and its thumbnail.
pub(crate) fn relative_files(rel: &str) -> [String; 3] {
    [rel.to_owned(), format!("{rel}{SIDECAR_SUFFIX}"), format!("{rel}{THUMBNAIL_SUFFIX}")]
}
