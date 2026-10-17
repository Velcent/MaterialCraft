//! Symmetry on the mesh (PT-36): a symmetric copy of a stamp paints the texels whose *mirrored* position lies under
//! the stamp, tested exactly like the direct copy (placement, occlusion, facing) with the texel's position and
//! normal transformed first. The settings panel, radial symmetry and the overlays come with the stroke-helpers
//! step; this is the hook the stamp stage runs every copy through.

use crate::mesh::Vec3;

/// Which copies a stroke paints besides itself. Positions are in the mesh's object space.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum SymmetrySpec {
    #[default]
    None,
    /// A mirror plane across object axis `axis` (0 = x, 1 = y, 2 = z) at `offset` along it.
    Mirror { axis: usize, offset: f32 },
}

/// One copy's transform of texel positions and normals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CopyTransform {
    Identity,
    Mirror { axis: usize, offset: f32 },
}

impl CopyTransform {
    #[inline]
    pub fn point(self, p: Vec3) -> Vec3 {
        match self {
            CopyTransform::Identity => p,
            CopyTransform::Mirror { axis, offset } => {
                let mut q = p;
                if let Some(v) = q.get_mut(axis) {
                    *v = 2.0 * offset - *v;
                }
                q
            }
        }
    }

    #[inline]
    pub fn normal(self, n: Vec3) -> Vec3 {
        match self {
            CopyTransform::Identity => n,
            CopyTransform::Mirror { axis, .. } => {
                let mut m = n;
                if let Some(v) = m.get_mut(axis) {
                    *v = -*v;
                }
                m
            }
        }
    }
}

impl SymmetrySpec {
    /// Every copy a stamp is applied with, the direct one first.
    pub fn copies(self) -> Vec<CopyTransform> {
        match self {
            SymmetrySpec::None => vec![CopyTransform::Identity],
            SymmetrySpec::Mirror { axis, offset } if axis < 3 && offset.is_finite() => vec![CopyTransform::Identity, CopyTransform::Mirror { axis, offset }],
            SymmetrySpec::Mirror { .. } => vec![CopyTransform::Identity],
        }
    }
}
