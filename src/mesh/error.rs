//! The error type for mesh checks and mesh processing.

use std::fmt;

/// What is wrong with a mesh, and in which part.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshError {
    /// Index into [`super::Mesh::parts`]; `None` when a single part was checked on its own.
    pub part: Option<usize>,
    pub kind: MeshErrorKind,
}

/// The individual problems a mesh can have.
#[derive(Debug, Clone, PartialEq)]
pub enum MeshErrorKind {
    /// A vertex attribute has a different length than `positions` (normals and tangents may also be empty).
    LengthMismatch {
        attribute: Attribute,
        expected: usize,
        found: usize,
    },
    /// A triangle refers to a vertex that does not exist.
    IndexOutOfRange {
        triangle: usize,
        index: u32,
        vertex_count: usize,
    },
    /// The part's material slot is not in [`super::Mesh::materials`].
    MaterialOutOfRange {
        material: usize,
        material_count: usize,
    },
    /// A vertex attribute holds NaN or infinity.
    NonFinite { attribute: Attribute, vertex: usize },
    /// A UV set was asked for that the part does not have.
    MissingUvSet { set: usize, available: usize },
}

/// A per-vertex array of [`super::MeshPart`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    Positions,
    Normals,
    Tangents,
    UvSet(usize),
}

impl MeshError {
    /// An error in a part that was checked on its own.
    pub fn new(kind: MeshErrorKind) -> MeshError {
        MeshError { part: None, kind }
    }

    /// The same error, attributed to part `index` of a mesh.
    pub fn in_part(self, index: usize) -> MeshError {
        MeshError {
            part: Some(index),
            ..self
        }
    }
}

impl fmt::Display for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Attribute::Positions => write!(f, "positions"),
            Attribute::Normals => write!(f, "normals"),
            Attribute::Tangents => write!(f, "tangents"),
            Attribute::UvSet(i) => write!(f, "UV set {i}"),
        }
    }
}

impl fmt::Display for MeshErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshErrorKind::LengthMismatch {
                attribute,
                expected,
                found,
            } => {
                write!(f, "{attribute} has {found} entries, expected {expected}")
            }
            MeshErrorKind::IndexOutOfRange {
                triangle,
                index,
                vertex_count,
            } => {
                write!(
                    f,
                    "triangle {triangle} uses vertex {index}, but there are only {vertex_count} vertices"
                )
            }
            MeshErrorKind::MaterialOutOfRange {
                material,
                material_count,
            } => {
                write!(
                    f,
                    "material slot {material} does not exist ({material_count} slots)"
                )
            }
            MeshErrorKind::NonFinite { attribute, vertex } => {
                write!(f, "{attribute} of vertex {vertex} is not a finite number")
            }
            MeshErrorKind::MissingUvSet { set, available } => {
                write!(f, "UV set {set} does not exist ({available} sets)")
            }
        }
    }
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.part {
            Some(p) => write!(f, "mesh part {p}: {}", self.kind),
            None => write!(f, "mesh part: {}", self.kind),
        }
    }
}

impl std::error::Error for MeshError {}
