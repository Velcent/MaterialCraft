//! [`StoreError`]: why the asset store refused or could not finish an operation. After any of them the library is as
//! it was before the operation (see [`StoreError::Incomplete`] for the one exception).

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum StoreError {
    /// The library is read-only (the starter library).
    ReadOnly { library: String },
    /// The library has no folder (session and project scopes live in memory).
    NoFolder { library: String },
    /// No library with this id.
    UnknownLibrary(String),
    /// A path that is not a plain library-relative path, or that leaves the library.
    BadPath { path: String, reason: &'static str },
    /// The file is not an asset (a sidecar, a thumbnail or another file).
    NotAnAsset(String),
    /// The asset is not there.
    Missing(PathBuf),
    /// The target exists; the store never replaces a file.
    Exists(PathBuf),
    /// The asset's sidecar can't be read, so it is not rewritten (it may hold the user's own fields).
    Sidecar { path: PathBuf, reason: String },
    /// A file operation failed; everything done before it was rolled back.
    Io { path: PathBuf, error: io::Error },
    /// A file operation failed and rolling back failed too: these files are left over (named in `leftovers`).
    Incomplete { cause: Box<StoreError>, leftovers: Vec<String> },
}

impl StoreError {
    pub(crate) fn io(path: &Path, error: io::Error) -> StoreError {
        StoreError::Io { path: path.to_path_buf(), error }
    }

    pub(crate) fn bad_path(path: &str, reason: &'static str) -> StoreError {
        StoreError::BadPath { path: path.to_owned(), reason }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::ReadOnly { library } => write!(f, "the library {library} is read-only"),
            StoreError::NoFolder { library } => write!(f, "the library {library} has no folder"),
            StoreError::UnknownLibrary(id) => write!(f, "there is no library {id}"),
            StoreError::BadPath { path, reason } => write!(f, "{path:?} {reason}"),
            StoreError::NotAnAsset(p) => write!(f, "{p} is not an asset file"),
            StoreError::Missing(p) => write!(f, "{} is not there", p.display()),
            StoreError::Exists(p) => write!(f, "{} already exists", p.display()),
            StoreError::Sidecar { path, reason } => write!(f, "the sidecar {} can't be read ({reason}); it is left as it is", path.display()),
            StoreError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            StoreError::Incomplete { cause, leftovers } => write!(f, "{cause}; rolling back left these files: {}", leftovers.join(", ")),
        }
    }
}

impl std::error::Error for StoreError {}
