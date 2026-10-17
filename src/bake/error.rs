//! Why a bake did not produce maps.

use crate::mesh::SurfaceError;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum BakeError {
    /// The texture set (low side) has no triangles.
    NoLowTriangles,
    /// The high mesh has no triangles.
    NoHighTriangles,
    /// The texture set's UVs are all zero or missing.
    NoUvs,
    /// The set's UV space could not be rasterised.
    Surface(SurfaceError),
    /// Nothing selected, or a size of 0.
    NothingToBake,
    /// The [`super::CancelToken`] was set.
    Cancelled,
}

impl fmt::Display for BakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BakeError::NoLowTriangles => f.write_str("the texture set has no triangles"),
            BakeError::NoHighTriangles => f.write_str("the high-poly mesh has no triangles"),
            BakeError::NoUvs => f.write_str("the low-poly mesh has no UV coordinates"),
            BakeError::Surface(e) => write!(f, "{e}"),
            BakeError::NothingToBake => f.write_str("no map selected"),
            BakeError::Cancelled => f.write_str("bake cancelled"),
        }
    }
}

impl std::error::Error for BakeError {}

impl From<SurfaceError> for BakeError {
    fn from(e: SurfaceError) -> BakeError {
        BakeError::Surface(e)
    }
}
