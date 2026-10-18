//! [`NewProjectError`]: why a project could not be made.

use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewProjectError {
    /// The file's type is not one MaterialCraft reads (yet); `what` names it.
    Unsupported { path: PathBuf, what: String },
    /// The file could not be read from disk.
    Io { path: PathBuf, message: String },
    /// The file was read but is not a usable mesh.
    Mesh { path: PathBuf, message: String },
    /// Parts of these materials have no UV coordinates, so nothing can be painted on them.
    NoUvs { materials: Vec<String> },
    /// The mesh has no triangles.
    Empty,
    /// The made document failed its own checks (a bug: reported rather than opened).
    Invalid(String),
}

impl fmt::Display for NewProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NewProjectError::Unsupported { path, what } => write!(f, "{}: {what}", path.display()),
            NewProjectError::Io { path, message } => write!(f, "cannot read {}: {message}", path.display()),
            NewProjectError::Mesh { path, message } => write!(f, "{}: {message}", path.display()),
            NewProjectError::NoUvs { materials } => write!(
                f,
                "the mesh has no UV coordinates on {}; unwrap it in a modelling program first (MaterialCraft does not generate UVs yet)",
                quoted(materials)
            ),
            NewProjectError::Empty => f.write_str("the mesh has no triangles"),
            NewProjectError::Invalid(m) => write!(f, "the new document is inconsistent: {m}"),
        }
    }
}

impl std::error::Error for NewProjectError {}

fn quoted(names: &[String]) -> String {
    let list: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    match list.len() {
        1 => format!("material {}", list.join("")),
        _ => format!("materials {}", list.join(", ")),
    }
}
