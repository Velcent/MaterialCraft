//! [`SceneFrame`]: the frame volume-based projections are expressed in: the bounds of the whole scene (every mesh
//! part, as Painter measures against the whole scene, not one texture set).

use crate::mesh::{Bounds, Mesh, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneFrame {
    pub centre: Vec3,
    /// Extent along x, y and z.
    pub size: Vec3,
}

impl SceneFrame {
    pub fn from_bounds(b: &Bounds) -> SceneFrame {
        SceneFrame { centre: b.centre(), size: b.size() }
    }

    /// The frame of every part of `mesh`, or `None` for a mesh without vertices.
    pub fn of_mesh(mesh: &Mesh) -> Option<SceneFrame> {
        mesh.bounds().map(|b| SceneFrame::from_bounds(&b))
    }

    /// The largest extent (at least a tiny positive value, so divisions stay finite): one *scene unit*, the unit
    /// of volume offsets and the size of a volume with scale 1 ([`super::volume`]).
    pub fn largest_extent(&self) -> f32 {
        let m = self.size.iter().copied().filter(|v| v.is_finite()).fold(0.0f32, f32::max);
        m.max(1e-6)
    }
}
