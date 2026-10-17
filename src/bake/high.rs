//! [`HighScene`]: the mesh the baker reads detail from, as a triangle list with shading normals and part ids,
//! plus its BVH. Build it once and bake every texture set against it.

use crate::mesh::{Bounds, Bvh, Hit, Mesh, Triangles};
use crate::v3::*;

/// One high-mesh triangle: corners, corner normals and the part it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighTriangle {
    pub p: [V; 3],
    pub n: [V; 3],
    pub group: u32,
}

/// Which triangles a ray may hit, by the side it sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Facing {
    Any,
    /// Only triangles whose front (counter-clockwise side) faces the ray.
    Front,
}

/// Where a ray met the high mesh.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct HighHit {
    pub t: f32,
    pub position: V,
    /// Interpolated corner normal (unit).
    pub normal: V,
    pub group: u32,
}

pub struct HighScene {
    tris: Vec<HighTriangle>,
    groups: Vec<String>,
    bvh: Bvh,
    bounds: (V, V),
}

impl Triangles for [HighTriangle] {
    fn triangle_count(&self) -> usize {
        self.len()
    }

    fn corners(&self, i: usize) -> [V; 3] {
        self.get(i).map(|t| t.p).unwrap_or_default()
    }
}

impl HighScene {
    /// Every part of `mesh`, one group per part (named after it). Parts without normals get flat ones.
    pub fn from_mesh(mesh: &Mesh) -> HighScene {
        let mut tris = Vec::new();
        let mut groups = Vec::new();
        for part in &mesh.parts {
            let group = groups.len() as u32;
            groups.push(part.name.clone());
            for t in &part.triangles {
                let idx = t.map(|i| i as usize);
                let (Some(&a), Some(&b), Some(&c)) = (part.positions.get(idx[0]), part.positions.get(idx[1]), part.positions.get(idx[2])) else {
                    continue;
                };
                let p = [a, b, c];
                let face = face_normal(&p);
                let n = idx.map(|i| part.normals.get(i).map_or(face, |&n| unit_or(n, face)));
                tris.push(HighTriangle { p, n, group });
            }
        }
        HighScene::from_triangles(tris, groups)
    }

    /// From a triangle list; `groups` names the groups the triangles refer to.
    pub fn from_triangles(tris: Vec<HighTriangle>, groups: Vec<String>) -> HighScene {
        let bvh = Bvh::build(tris.as_slice());
        let bounds = Bounds::of_points(tris.iter().flat_map(|t| t.p.iter())).map_or(([0.0; 3], [0.0; 3]), |b| (b.min, b.max));
        HighScene { tris, groups, bvh, bounds }
    }

    pub fn triangle_count(&self) -> usize {
        self.tris.len()
    }

    /// Group names; [`HighTriangle::group`] indexes this.
    pub fn groups(&self) -> &[String] {
        &self.groups
    }

    /// Axis-aligned bounds (min, max); zero when empty.
    pub fn bounds(&self) -> (V, V) {
        self.bounds
    }

    /// The length that relative distances are multiplied with: the bounding box diagonal.
    pub fn scale(&self) -> f32 {
        len(sub(self.bounds.1, self.bounds.0)).max(1e-6)
    }

    /// The nearest hit of `o + t d` with `tmin <= t < tmax` on a triangle facing as asked.
    pub(crate) fn trace(&self, o: V, d: V, tmin: f32, tmax: f32, facing: Facing) -> Option<HighHit> {
        let tris = self.tris.as_slice();
        let accept = |h: &Hit| {
            if h.t < tmin {
                return false;
            }
            let Some(tri) = tris.get(h.tri as usize) else {
                return false;
            };
            match facing {
                Facing::Any => true,
                Facing::Front => dot(face_normal(&tri.p), d) < 0.0,
            }
        };
        let hit = self.bvh.intersect_where(tris, o, d, tmax, accept)?;
        let tri = tris.get(hit.tri as usize)?;
        let w = [1.0 - hit.u - hit.v, hit.u, hit.v];
        let n = add(add(mul(tri.n[0], w[0]), mul(tri.n[1], w[1])), mul(tri.n[2], w[2]));
        Some(HighHit { t: hit.t, position: add(o, mul(d, hit.t)), normal: unit_or(n, face_normal(&tri.p)), group: tri.group })
    }
}

/// Unit normal of a counter-clockwise triangle ([0, 0, 1] when it has no area).
fn face_normal(p: &[V; 3]) -> V {
    norm(cross(sub(p[1], p[0]), sub(p[2], p[0])))
}

fn unit_or(v: V, fallback: V) -> V {
    let l = len(v);
    if l > 1e-12 && l.is_finite() { mul(v, 1.0 / l) } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(z: f32, flip: bool, group: u32) -> [HighTriangle; 2] {
        let (a, b, c, d) = ([-1.0, -1.0, z], [1.0, -1.0, z], [1.0, 1.0, z], [-1.0, 1.0, z]);
        let n = if flip { [0.0, 0.0, -1.0] } else { [0.0, 0.0, 1.0] };
        let tri = |p: [V; 3]| HighTriangle { p: if flip { [p[0], p[2], p[1]] } else { p }, n: [n; 3], group };
        [tri([a, b, c]), tri([a, c, d])]
    }

    #[test]
    fn traces_by_facing() {
        let mut tris = square(0.0, false, 0).to_vec();
        tris.extend(square(-1.0, true, 1));
        let scene = HighScene::from_triangles(tris, vec!["top".into(), "bottom".into()]);
        assert_eq!(scene.bounds(), ([-1.0, -1.0, -1.0], [1.0, 1.0, 0.0]));
        let down = [0.0, 0.0, -1.0];
        let hit = scene.trace([0.1, 0.2, 1.0], down, 0.0, 10.0, Facing::Any).unwrap();
        assert!((hit.t - 1.0).abs() < 1e-5 && hit.group == 0);
        assert!((hit.position[2]).abs() < 1e-5 && hit.normal == [0.0, 0.0, 1.0]);
        // the bottom square faces down: from above it is a back face, found only when any facing is accepted
        assert_eq!(scene.trace([0.1, 0.2, 1.0], down, 1.5, 10.0, Facing::Any).unwrap().group, 1);
        assert!(scene.trace([0.1, 0.2, 1.0], down, 1.5, 10.0, Facing::Front).is_none());
        assert!(scene.trace([0.1, 0.2, 1.0], down, 0.0, 0.5, Facing::Any).is_none());
    }
}
