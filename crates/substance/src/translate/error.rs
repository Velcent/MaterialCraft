//! [`ImportError`]: why a project could not be imported at all. Problems with single parts (one channel,
//! one baked map) do not fail the import; they become entries of the import report.

use crate::decode::DecodeError;
use crate::project::ProjectError;
use materialcraft::paint::ImageError;
use std::fmt;

#[derive(Debug)]
pub enum ImportError {
    /// The file could not be opened or a required part could not be read.
    Project(ProjectError),
    /// A required record (mesh, document) could not be decoded.
    Decode { part: &'static str, source: DecodeError },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Project(e) => write!(f, "{e}"),
            ImportError::Decode { part, source } => write!(f, "{part}: {source}"),
        }
    }
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImportError::Project(e) => Some(e),
            ImportError::Decode { source, .. } => Some(source),
        }
    }
}

impl From<ProjectError> for ImportError {
    fn from(e: ProjectError) -> Self {
        ImportError::Project(e)
    }
}

/// Why one part (a channel, a baked map) was left out. Becomes a report note; the import goes on.
#[derive(Debug)]
pub enum PartError {
    Project(ProjectError),
    Decode(DecodeError),
    Image(ImageError),
    /// A pixel format we have no conversion for.
    UnknownFormat(String),
    /// A referenced resource is not in the file.
    Missing(String),
}

impl fmt::Display for PartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PartError::Project(e) => write!(f, "{e}"),
            PartError::Decode(e) => write!(f, "{e}"),
            PartError::Image(e) => write!(f, "{e}"),
            PartError::UnknownFormat(name) => write!(f, "unsupported pixel format {name}"),
            PartError::Missing(what) => write!(f, "{what} is not in the file"),
        }
    }
}

impl From<ProjectError> for PartError {
    fn from(e: ProjectError) -> Self {
        PartError::Project(e)
    }
}

impl From<DecodeError> for PartError {
    fn from(e: DecodeError) -> Self {
        PartError::Decode(e)
    }
}

impl From<ImageError> for PartError {
    fn from(e: ImageError) -> Self {
        PartError::Image(e)
    }
}
