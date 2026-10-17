//! [`uv_pick`]: picks in UV space, for the 2D view (the editor's 2D view calls it once it takes polygon fill input).

use super::Picked;
use super::region::{PickRegion, bounds, in_triangle, touches};
use crate::mesh::Mesh;

/// The triangles of material slot `material`'s parts that `region` (UV units, the mesh's convention: v down)
/// picks in UV set `uv_set`: a point the first triangle whose UVs contain it, an area every triangle it touches.
/// Parts without that UV set are left out.
pub fn uv_pick(mesh: &Mesh, material: usize, uv_set: usize, region: &PickRegion) -> Picked {
    let mut picked = Picked::new();
    let outline = region.outline();
    let outline_bounds = outline.as_deref().map(bounds);
    for (pi, part) in mesh.parts.iter().enumerate() {
        if part.material != material {
            continue;
        }
        let Some(uvs) = part.uv_sets.get(uv_set).filter(|u| u.len() == part.positions.len()) else { continue };
        for (t, tri) in part.triangles.iter().enumerate() {
            let [Some(a), Some(b), Some(c)] = tri.map(|i| uvs.get(i as usize).copied()) else { continue };
            let hit = match (region, &outline, outline_bounds) {
                (PickRegion::Point(q), _, _) => in_triangle([a, b, c], *q),
                (_, Some(o), Some(ob)) => touches([a, b, c], o, ob),
                _ => false,
            };
            if hit {
                picked.entry(pi).or_default().insert(t as u32);
                if matches!(region, PickRegion::Point(_)) {
                    return picked;
                }
            }
        }
    }
    picked
}
