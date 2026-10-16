//! [`GltfError`]: why a mesh could not be written as, or read from, glTF.

use crate::mesh::MeshError;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum GltfError {
    /// The GLB container (header or chunks) is malformed.
    Container(String),
    /// The JSON chunk is not valid JSON.
    Json(String),
    /// The glTF content breaks the spec. `at` is a JSON path such as `accessors[3].count`.
    Invalid { at: String, message: String },
    /// Valid glTF using a feature this reader does not handle (external buffers, sparse
    /// accessors, line primitives, required extensions, …).
    Unsupported(String),
    /// The mesh to write (or the mesh read) breaks a [`crate::mesh::Mesh`] invariant.
    Mesh(MeshError),
}

impl GltfError {
    pub(crate) fn invalid(at: impl Into<String>, message: impl Into<String>) -> GltfError {
        GltfError::Invalid {
            at: at.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for GltfError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GltfError::Container(m) => write!(f, "invalid GLB: {m}"),
            GltfError::Json(m) => write!(f, "invalid glTF JSON: {m}"),
            GltfError::Invalid { at, message } => write!(f, "invalid glTF, {at}: {message}"),
            GltfError::Unsupported(m) => write!(f, "unsupported glTF feature: {m}"),
            GltfError::Mesh(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GltfError {}

impl From<MeshError> for GltfError {
    fn from(e: MeshError) -> GltfError {
        GltfError::Mesh(e)
    }
}
