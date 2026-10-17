//! [`LowSurface`]: the texture set's surface under every texel (from [`SurfaceMap`]s), with the frames the bakers
//! need: position, shading normal, ray direction and the tangent frame of the normal map.

use super::error::BakeError;
use super::settings::TangentSource;
use crate::mesh::{Bounds, Mesh, MeshPart, SurfaceKey, SurfaceMap, Topology, mesh_fingerprint};
use crate::v3::*;

/// Which part of a mesh one bake covers: the parts on these material slots (Painter can merge several slots into
/// one texture set), unwrapped by UV set `uv_set`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureSetRef {
    pub materials: Vec<usize>,
    pub uv_set: usize,
}

impl TextureSetRef {
    /// One material slot, UV set 0.
    pub fn slot(material: usize) -> TextureSetRef {
        TextureSetRef { materials: vec![material], uv_set: 0 }
    }

    fn contains(&self, part: &MeshPart) -> bool {
        self.materials.contains(&part.material)
    }
}

/// The low surface under one texel.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct LowTexel {
    pub position: V,
    /// Unit shading normal.
    pub normal: V,
    /// Unit direction rays are cast along (outwards).
    pub ray: V,
    /// The normal map's frame as the renderer rebuilds it per pixel: interpolated tangent and normal (not
    /// normalised) and `sign * cross(normal, tangent)`.
    pub frame: [V; 3],
}

/// Per-vertex attributes of one part of the set.
struct PartFrames {
    normals: Vec<V>,
    tangents: Vec<[f32; 4]>,
    rays: Vec<V>,
}

pub(crate) struct LowSurface<'a> {
    mesh: &'a Mesh,
    maps: Vec<SurfaceMap>,
    frames: Vec<Option<PartFrames>>,
    size: usize,
}

impl<'a> LowSurface<'a> {
    pub fn build(mesh: &'a Mesh, set: &TextureSetRef, size: usize, tangents: TangentSource, average_normals: bool) -> Result<LowSurface<'a>, BakeError> {
        let parts: Vec<&MeshPart> = mesh.parts.iter().filter(|p| set.contains(p)).collect();
        if parts.iter().all(|p| p.triangles.is_empty()) {
            return Err(BakeError::NoLowTriangles);
        }
        let has_uvs = parts.iter().any(|p| p.uv_sets.get(set.uv_set).is_some_and(|uv| uv.iter().any(|u| *u != [0.0, 0.0])));
        if !has_uvs {
            return Err(BakeError::NoUvs);
        }
        let mut materials = set.materials.clone();
        materials.sort_unstable();
        materials.dedup();
        let mut maps = Vec::new();
        for &material in &materials {
            if !mesh.parts.iter().any(|p| p.material == material) {
                continue;
            }
            let side = u32::try_from(size).unwrap_or(u32::MAX);
            let key = SurfaceKey { material, uv_set: set.uv_set, width: side, height: side, gutter: 0, mesh_revision: mesh_fingerprint(mesh, material) };
            maps.push(SurfaceMap::build(mesh, key)?);
        }
        let frames = mesh.parts.iter().map(|p| set.contains(p).then(|| PartFrames::of(p, set.uv_set, tangents, average_normals))).collect();
        Ok(LowSurface { mesh, maps, frames, size })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// The surface under texel `i` (row-major), `None` where the set does not cover it.
    pub fn texel(&self, i: usize) -> Option<LowTexel> {
        let sample = self.maps.iter().filter_map(|m| m.sample_index(i)).filter(|s| s.coverage > 0.0).max_by(|a, b| a.coverage.total_cmp(&b.coverage))?;
        let part = self.mesh.parts.get(sample.part)?;
        let frames = self.frames.get(sample.part)?.as_ref()?;
        let tri = part.triangles.get(sample.triangle)?.map(|v| v as usize);
        let b = sample.barycentric;
        let lerp3 = |v: [V; 3]| add(add(mul(v[0], b[0]), mul(v[1], b[1])), mul(v[2], b[2]));
        let at = |list: &[V]| -> Option<V> { Some(lerp3([*list.get(tri[0])?, *list.get(tri[1])?, *list.get(tri[2])?])) };
        let corners = [*part.positions.get(tri[0])?, *part.positions.get(tri[1])?, *part.positions.get(tri[2])?];
        if degenerate(corners) {
            return None; // no area in 3D (e.g. a sphere's pole): no frame; dilation fills the texel
        }
        let position = lerp3(corners);
        let n = at(&frames.normals)?;
        let normal = unit(n, sample.normal);
        let ray = unit(at(&frames.rays)?, normal);
        let [t0, t1, t2] = tri.map(|v| frames.tangents.get(v).copied().unwrap_or([1.0, 0.0, 0.0, 1.0]));
        let tangent = lerp3([[t0[0], t0[1], t0[2]], [t1[0], t1[1], t1[2]], [t2[0], t2[1], t2[2]]]);
        let w = t0[3] * b[0] + t1[3] * b[1] + t2[3] * b[2];
        let sign = if w < 0.0 { -1.0 } else { 1.0 };
        let bitangent = mul(cross(n, tangent), sign);
        Some(LowTexel { position, normal, ray, frame: [tangent, bitangent, n] })
    }

    /// Bounds of the set's vertices.
    pub fn set_bounds(&self) -> Option<Bounds> {
        self.mesh.parts.iter().zip(&self.frames).filter(|(_, f)| f.is_some()).filter_map(|(p, _)| p.bounds()).reduce(Bounds::union)
    }

    /// Bounds of the whole mesh (every set).
    pub fn scene_bounds(&self) -> Option<Bounds> {
        self.mesh.bounds()
    }
}

impl PartFrames {
    fn of(part: &MeshPart, uv_set: usize, tangents: TangentSource, average_normals: bool) -> PartFrames {
        let n = part.positions.len();
        let mut work = part.clone();
        if work.normals.len() != n {
            work.generate_normals();
        }
        let keep = tangents == TangentSource::Mesh && work.tangents.len() == n;
        if !keep && work.generate_tangents(uv_set).is_err() {
            work.tangents = vec![[1.0, 0.0, 0.0, 1.0]; n];
        }
        let rays = if average_normals { averaged_normals(&work, uv_set) } else { work.normals.clone() };
        PartFrames { normals: work.normals, tangents: work.tangents, rays }
    }
}

/// Vertex normals averaged over all vertices at the same position (welded), so hard edges cast no gaps.
fn averaged_normals(part: &MeshPart, uv_set: usize) -> Vec<V> {
    let topo = Topology::of_part(part, uv_set);
    let mut sum = vec![[0.0f32; 3]; topo.welded_vertex_count()];
    let welded: Vec<Option<usize>> = (0..part.positions.len()).map(|v| topo.welded_vertex(v).map(|w| w as usize)).collect();
    for (v, w) in welded.iter().enumerate() {
        if let (Some(w), Some(n)) = (w, part.normals.get(v))
            && let Some(s) = sum.get_mut(*w)
        {
            *s = add(*s, unit(*n, [0.0; 3]));
        }
    }
    welded
        .iter()
        .enumerate()
        .map(|(v, w)| {
            let own = part.normals.get(v).copied().unwrap_or([0.0, 0.0, 1.0]);
            w.and_then(|w| sum.get(w)).map_or(own, |s| unit(*s, own))
        })
        .collect()
}

/// A sliver or point: its area is tiny against its edge lengths.
fn degenerate(p: [V; 3]) -> bool {
    let (e1, e2) = (sub(p[1], p[0]), sub(p[2], p[0]));
    let area = len(cross(e1, e2));
    area < 1e-4 * len(e1) * len(e2) || area < 1e-12
}

fn unit(v: V, fallback: V) -> V {
    let l = len(v);
    if l > 1e-12 && l.is_finite() { mul(v, 1.0 / l) } else { fallback }
}
