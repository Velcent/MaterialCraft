//! [`FormatError`]: everything that can go wrong reading or writing a `.mcpaint` file.

use crate::mesh::MeshError;
use crate::mesh::gltf::GltfError;
use crate::paint::validate::DocError;
use crate::zip::ZipError;
use std::fmt;

/// Why a `.mcpaint` file could not be read or written. A corrupt file always gives an error, never a
/// panic.
#[derive(Debug)]
pub enum FormatError {
    Io(std::io::Error),
    /// The ZIP container is broken or uses a feature we do not read.
    Zip(ZipError),
    /// An archive entry the manifest needs is missing.
    MissingEntry(String),
    /// `document.json` is not valid JSON.
    Json(String),
    /// Not a `.mcpaint` document (`format` is not `"mcpaint"`).
    NotMcpaint,
    /// Not a fragment (`.mcfrag`: `format` is not `"mcfrag"`).
    NotFragment,
    /// The file has a newer major `format_version` than this reader supports.
    NewerVersion {
        found: String,
        supported: String,
    },
    /// The manifest is valid JSON but does not follow the spec. `at` is a path such as
    /// `texture_sets[0].layers[2].blend`.
    Manifest {
        at: String,
        message: String,
    },
    /// An image entry could not be encoded or decoded, or does not match its declaration.
    Image {
        entry: String,
        message: String,
    },
    /// The mesh could not be encoded or decoded.
    Mesh(GltfError),
    /// The mesh breaks an invariant (checked before saving).
    InvalidMesh(MeshError),
    /// The document breaks an invariant (checked before saving and after loading).
    Invalid(DocError),
    /// A valid document this version cannot store or read, e.g. an image too large for PNG.
    Unsupported(String),
    /// Loading would decode more pixel data than [`super::LoadOptions::max_image_bytes`].
    TooLarge {
        limit: u64,
    },
}

impl FormatError {
    pub(crate) fn manifest(at: impl Into<String>, message: impl Into<String>) -> FormatError {
        FormatError::Manifest { at: at.into(), message: message.into() }
    }

    pub(crate) fn image(entry: impl Into<String>, message: impl Into<String>) -> FormatError {
        FormatError::Image { entry: entry.into(), message: message.into() }
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FormatError::Io(e) => write!(f, "I/O error: {e}"),
            FormatError::Zip(e) => write!(f, "{e}"),
            FormatError::MissingEntry(n) => write!(f, "the document has no entry {n:?}"),
            FormatError::Json(m) => write!(f, "document.json: {m}"),
            FormatError::NotMcpaint => write!(f, "not a .mcpaint document"),
            FormatError::NotFragment => write!(f, "not a smart material or smart mask (.mcfrag) file"),
            FormatError::NewerVersion { found, supported } => write!(
                f,
                "the document has format version {found}, this program reads up to {supported}; \
                 a newer MaterialCraft is needed"
            ),
            FormatError::Manifest { at, message } => write!(f, "document.json, {at}: {message}"),
            FormatError::Image { entry, message } => write!(f, "image {entry:?}: {message}"),
            FormatError::Mesh(e) => write!(f, "mesh: {e}"),
            FormatError::InvalidMesh(e) => write!(f, "invalid mesh: {e}"),
            FormatError::Invalid(e) => write!(f, "invalid document: {e}"),
            FormatError::Unsupported(m) => write!(f, "unsupported: {m}"),
            FormatError::TooLarge { limit } => {
                write!(f, "the document holds more than {limit} bytes of images")
            }
        }
    }
}

impl std::error::Error for FormatError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FormatError::Io(e) => Some(e),
            FormatError::Zip(e) => Some(e),
            FormatError::Mesh(e) => Some(e),
            FormatError::Invalid(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FormatError {
    fn from(e: std::io::Error) -> FormatError {
        FormatError::Io(e)
    }
}

impl From<ZipError> for FormatError {
    fn from(e: ZipError) -> FormatError {
        match e {
            ZipError::Io(e) => FormatError::Io(e),
            ZipError::NotFound(n) => FormatError::MissingEntry(n),
            e => FormatError::Zip(e),
        }
    }
}

impl From<GltfError> for FormatError {
    fn from(e: GltfError) -> FormatError {
        FormatError::Mesh(e)
    }
}

impl From<DocError> for FormatError {
    fn from(e: DocError) -> FormatError {
        FormatError::Invalid(e)
    }
}
