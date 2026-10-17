//! The triangles of one texture set, gathered from its parts with everything a texel needs.

use super::super::normals::smooth_normals;
use super::super::tangents::tangent_frames;
use super::super::{Mesh, MeshPart, Topology, Vec3};
use super::SurfaceError;

/// One triangle of the set with its corner attributes, as the surface map stores it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SurfaceTriangle {
    pub p: [Vec3; 3],
    pub n: [Vec3; 3],
    pub t: [[f32; 4]; 3],
    pub uv: [[f32; 2]; 3],
    /// Index into `Mesh::parts`.
    pub part: u32,
    /// Index into that part's `triangles`.
    pub triangle: u32,
    /// UV island, numbered across the set (the parts' islands one after another).
    pub island: u32,
}

/// All triangles of the set's parts that have valid indices, and how many islands they form.
pub(super) fn gather(mesh: &Mesh, material: usize, uv_set: usize) -> Result<(Vec<SurfaceTriangle>, u32), SurfaceError> {
    if material >= mesh.materials.len() && !mesh.parts.iter().any(|p| p.material == material) {
        return Err(SurfaceError::NoSuchMaterial { material });
    }
    let mut tris = Vec::new();
    let mut islands = 0u32;
    for (pi, part) in mesh.parts.iter().enumerate().filter(|(_, p)| p.material == material) {
        let uvs = part.uv_sets.get(uv_set).filter(|uv| uv.len() == part.positions.len()).ok_or(SurfaceError::MissingUvSet { part: pi, set: uv_set })?;
        let topo = Topology::of_part(part, uv_set);
        let (normals, tangents) = frames(part, uvs);
        for (ti, tri) in part.triangles.iter().enumerate() {
            let Some(island) = topo.islands().of(ti) else {
                continue;
            };
            let corner = |k: usize| tri.get(k).map(|&i| i as usize);
            let at = |k: usize| -> Option<(Vec3, Vec3, [f32; 4], [f32; 2])> {
                let i = corner(k)?;
                Some((*part.positions.get(i)?, *normals.get(i)?, *tangents.get(i)?, *uvs.get(i)?))
            };
            let (Some(a), Some(b), Some(c)) = (at(0), at(1), at(2)) else {
                continue;
            };
            tris.push(SurfaceTriangle {
                p: [a.0, b.0, c.0],
                n: [a.1, b.1, c.1],
                t: [a.2, b.2, c.2],
                uv: [a.3, b.3, c.3],
                part: pi as u32,
                triangle: ti as u32,
                island: islands.saturating_add(island),
            });
        }
        islands = islands.saturating_add(topo.islands().count());
    }
    Ok((tris, islands))
}

/// The part's normals and tangents, generated when missing or of the wrong length (without changing the part).
fn frames(part: &MeshPart, uvs: &[[f32; 2]]) -> (Vec<Vec3>, Vec<[f32; 4]>) {
    let n = part.positions.len();
    let normals = if part.normals.len() == n { part.normals.clone() } else { smooth_normals(&part.positions, &part.triangles) };
    let tangents = if part.tangents.len() == n { part.tangents.clone() } else { tangent_frames(&part.positions, &normals, uvs, &part.triangles) };
    (normals, tangents)
}
