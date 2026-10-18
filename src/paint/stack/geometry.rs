//! Geometry masks at composite time: a layer's or folder's include list of mesh parts becomes a per-texel coverage
//! (1 on an included part, 0 elsewhere and outside the set) from the surface map's part ids, multiplied into the
//! layer's strength. Without a surface map, and for UV-tile masks (no UDIMs yet), the layer is applied unmasked and
//! reported.

use super::Compositor;
use crate::paint::geometry_mask::GeometryMask;
use crate::paint::image::Rect;
use crate::paint::layer::Layer;

impl Compositor<'_> {
    /// Whether `mask` can be applied: a part list with the set's surface map.
    pub(super) fn geometry_mask_computable(&self, mask: &GeometryMask) -> bool {
        matches!(mask, GeometryMask::Parts(_)) && self.fills.surface.is_some()
    }

    /// Per texel of `r`: 1 where `layer`'s geometry mask includes the part under the texel, else 0; `None` where the
    /// layer has no geometry mask or it cannot be applied.
    pub(super) fn geometry_coverage(&self, layer: &Layer, r: Rect) -> Option<Vec<f32>> {
        let mask = layer.geometry_mask.as_ref().filter(|m| self.geometry_mask_computable(m))?;
        let surface = self.fills.surface?;
        let included: Vec<bool> = (0..).map_while(|p| surface.part_name(p)).map(|name| mask.includes_part(name)).collect();
        let w = surface.width();
        let mut out = vec![0.0; r.area()];
        if !included.contains(&true) {
            return Some(out);
        }
        for (i, o) in out.iter_mut().enumerate() {
            let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
            if x < w
                && let Some((part, _, _)) = surface.triangle_index(y * w + x)
                && included.get(part).copied().unwrap_or(false)
            {
                *o = 1.0;
            }
        }
        Some(out)
    }
}
