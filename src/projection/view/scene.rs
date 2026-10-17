//! [`ViewScene`]: what the CPU view projector keeps per mesh: a BVH per part, the fit the viewport places the
//! model with, and per part the UV set picks report and whether the part is shown.

use super::cpu::CpuViewProjector;
use crate::camera::Camera;
use crate::mesh::{Bvh, Fit, Mesh};

pub(super) struct PartScene {
    pub bvh: Bvh,
    /// Triangle count the BVH was built from (to recognise a different mesh).
    pub triangles: usize,
    pub uv_set: usize,
    pub shown: bool,
}

/// The per-mesh half of [`CpuViewProjector`]; build it once per mesh (building is the expensive part) and pair it
/// with each frame's camera through [`ViewScene::projector`].
pub struct ViewScene {
    pub(super) parts: Vec<PartScene>,
    pub(super) fit: Fit,
    /// A small distance in object units: occlusion rays start this far off the surface, so a point does not
    /// occlude itself.
    pub(super) epsilon: f32,
}

impl ViewScene {
    /// Every part of `mesh` shown, picks reporting UV set 0. `fit` places the mesh in the world the camera looks at
    /// (the viewport draws `mesh.fit()`).
    pub fn new(mesh: &Mesh, fit: Fit) -> ViewScene {
        let parts = mesh.parts.iter().map(|p| PartScene { bvh: Bvh::build(p), triangles: p.triangles.len(), uv_set: 0, shown: true }).collect();
        let radius = mesh.bounding_sphere().map_or(1.0, |s| s.radius);
        let epsilon = if radius.is_finite() && radius > 0.0 { radius * 1e-4 } else { 1e-6 };
        ViewScene { parts, fit, epsilon }
    }

    /// Picks on `part` report texture coordinates of UV set `set` (the set its texture set paints on).
    pub fn set_uv_set(&mut self, part: usize, set: usize) {
        if let Some(p) = self.parts.get_mut(part) {
            p.uv_set = set;
        }
    }

    /// Hidden parts are neither picked nor occlude.
    pub fn set_shown(&mut self, part: usize, shown: bool) {
        if let Some(p) = self.parts.get_mut(part) {
            p.shown = shown;
        }
    }

    /// False for hidden parts (and parts the mesh does not have).
    pub fn is_shown(&self, part: usize) -> bool {
        self.parts.get(part).is_some_and(|p| p.shown)
    }

    pub fn fit(&self) -> Fit {
        self.fit
    }

    /// True if the scene was built from a mesh with the same parts and triangle counts as `mesh`.
    pub fn matches(&self, mesh: &Mesh) -> bool {
        self.parts.len() == mesh.parts.len() && self.parts.iter().zip(&mesh.parts).all(|(s, p)| s.triangles == p.triangles.len())
    }

    /// The projector for one camera snapshot; `mesh` must be the mesh the scene was built from (`None` if it
    /// visibly is not, see [`ViewScene::matches`]).
    pub fn projector<'a>(&'a self, mesh: &'a Mesh, camera: Camera) -> Option<CpuViewProjector<'a>> {
        self.matches(mesh).then(|| CpuViewProjector::new(self, mesh, camera))
    }
}
