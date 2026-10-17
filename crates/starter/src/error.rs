//! [`StarterError`]: why the starter library could not be built.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum StarterError {
    /// A recipe could not make its asset (the text says why).
    Recipe { asset: String, reason: String },
    /// Writing the library failed.
    Io { path: PathBuf, error: io::Error },
    /// The target folder exists and is not a starter library; it is never overwritten.
    NotStarter(PathBuf),
}

impl StarterError {
    pub(crate) fn recipe(asset: &str, reason: impl fmt::Display) -> StarterError {
        StarterError::Recipe { asset: asset.to_owned(), reason: reason.to_string() }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, error: io::Error) -> StarterError {
        StarterError::Io { path: path.into(), error }
    }
}

impl fmt::Display for StarterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StarterError::Recipe { asset, reason } => write!(f, "{asset}: {reason}"),
            StarterError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            StarterError::NotStarter(p) => write!(f, "{} exists and is not a starter library; choose another folder", p.display()),
        }
    }
}

impl std::error::Error for StarterError {}
