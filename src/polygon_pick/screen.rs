//! [`ScreenPick`]: picks in the 3D view through a [`ViewProjector`].

use super::Picked;
use super::region::{PickRegion, bounds, touches};
use crate::mesh::{Mesh, Triangles};
use crate::projection::ViewProjector;

/// Picks triangles of one material slot's parts through a view.
pub struct ScreenPick<'a> {
    pub projector: &'a dyn ViewProjector,
    pub mesh: &'a Mesh,
    /// The material slot (texture set) whose parts can be picked.
    pub material: usize,
    /// A rectangle or lasso picks only triangles whose centre the camera sees (not hidden behind other surfaces).
    /// A click always picks the nearest triangle under the pointer.
    pub visible_only: bool,
    /// Parts the view shows (hidden parts are not picked).
    pub shown: &'a dyn Fn(usize) -> bool,
}

impl ScreenPick<'_> {
    /// The triangles `region` (screen units) picks, before growing.
    pub fn pick(&self, region: &PickRegion) -> Picked {
        let mut picked = Picked::new();
        if let PickRegion::Point(q) = region {
            if let Some(hit) = self.projector.pick(*q)
                && self.mesh.parts.get(hit.part).is_some_and(|p| p.material == self.material)
            {
                picked.entry(hit.part).or_default().insert(hit.triangle);
            }
            return picked;
        }
        let Some(outline) = region.outline() else { return picked };
        let outline_bounds = bounds(&outline);
        for (pi, part) in self.mesh.parts.iter().enumerate() {
            if part.material != self.material || !(self.shown)(pi) {
                continue;
            }
            for t in 0..part.triangles.len() {
                let corners = part.corners(t);
                let screen = corners.map(|c| self.projector.to_screen(c).map(|s| s.pos));
                let [Some(a), Some(b), Some(c)] = screen else { continue };
                if !touches([a, b, c], &outline, outline_bounds) {
                    continue;
                }
                if self.visible_only {
                    let centre = std::array::from_fn(|k| corners.iter().map(|c| c.get(k).copied().unwrap_or(0.0)).sum::<f32>() / 3.0);
                    if self.projector.occluded(centre) {
                        continue;
                    }
                }
                picked.entry(pi).or_default().insert(t as u32);
            }
        }
        picked
    }
}
