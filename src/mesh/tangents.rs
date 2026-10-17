//! Per-vertex tangent generation from a UV set.
//!
//! Convention (same as glTF and MikkTSpace with a top-left UV origin): `tangent.xyz` points along +u and the
//! bitangent `cross(normal, tangent.xyz) * w` points along -v, i.e. "up" in the texture. `w` is -1 on mirrored
//! UV islands.

use super::derivatives::TriangleFrame;
use super::error::{MeshError, MeshErrorKind};
use super::normals::{smooth_normals, unit_or};
use super::{MeshPart, Vec3};
use crate::v3::{add, cross, dot, mul, sub};

impl MeshPart {
    /// Replaces `tangents` with tangents derived from UV set `uv_set`, generating normals first if there are none.
    pub fn generate_tangents(&mut self, uv_set: usize) -> Result<(), MeshError> {
        let available = self.uv_sets.len();
        let uvs = self.uv_sets.get(uv_set).ok_or(MeshError::new(MeshErrorKind::MissingUvSet { set: uv_set, available }))?;
        if self.normals.len() != self.positions.len() {
            self.normals = smooth_normals(&self.positions, &self.triangles);
        }
        self.tangents = tangent_frames(&self.positions, &self.normals, uvs, &self.triangles);
        Ok(())
    }

    /// Generates tangents only when the part has none.
    pub fn ensure_tangents(&mut self, uv_set: usize) -> Result<(), MeshError> {
        if self.tangents.is_empty() {
            self.generate_tangents(uv_set)?;
        }
        Ok(())
    }
}

/// Unit tangents orthogonal to `normals`, with handedness in w. Vertices whose triangles all have degenerate UVs
/// get an arbitrary tangent perpendicular to their normal and w = 1. Missing normals or UVs count as degenerate.
pub(super) fn tangent_frames(positions: &[Vec3], normals: &[Vec3], uvs: &[[f32; 2]], triangles: &[[u32; 3]]) -> Vec<[f32; 4]> {
    let n = positions.len();
    let mut along_u = vec![[0.0f32; 3]; n];
    let mut along_v = vec![[0.0f32; 3]; n];
    for &tri in triangles {
        let Some(f) = TriangleFrame::of_triangle(positions, uvs, tri) else {
            continue;
        };
        // unit directions weighted by area, so tiny or heavily stretched triangles do not dominate
        let du = mul(unit_or(f.dp_du, [0.0; 3]), f.area);
        let dv = mul(unit_or(f.dp_dv, [0.0; 3]), f.area);
        for i in tri {
            let i = i as usize;
            along_u[i] = add(along_u[i], du);
            along_v[i] = add(along_v[i], dv);
        }
    }
    (0..n)
        .map(|i| {
            let normal = unit_or(normals.get(i).copied().unwrap_or([0.0; 3]), [0.0, 0.0, 1.0]);
            vertex_tangent(normal, along_u[i], along_v[i])
        })
        .collect()
}

/// Gram-Schmidt of the accumulated +u direction against the normal, with fallbacks for degenerate input.
fn vertex_tangent(normal: Vec3, along_u: Vec3, along_v: Vec3) -> [f32; 4] {
    let up = mul(along_v, -1.0); // texture "up" is -v
    let mut t = unit_or(reject(along_u, normal), [0.0; 3]);
    if t == [0.0; 3] {
        // no usable u direction: derive it from v so that cross(normal, t) = up
        t = unit_or(reject(cross(up, normal), normal), [0.0; 3]);
    }
    if t == [0.0; 3] {
        t = any_perpendicular(normal);
    }
    let w = if dot(cross(normal, t), up) < 0.0 { -1.0 } else { 1.0 };
    [t[0], t[1], t[2], w]
}

/// `v` with its component along the unit vector `n` removed.
fn reject(v: Vec3, n: Vec3) -> Vec3 {
    sub(v, mul(n, dot(v, n)))
}

/// A unit vector perpendicular to the unit vector `n`.
fn any_perpendicular(n: Vec3) -> Vec3 {
    let axis = if n[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    unit_or(reject(axis, n), [1.0, 0.0, 0.0])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::{close, mirrored_quad, quad};

    fn assert_unit_and_perpendicular(p: &MeshPart) {
        for (t, n) in p.tangents.iter().zip(&p.normals) {
            let xyz = [t[0], t[1], t[2]];
            assert!((dot(xyz, xyz) - 1.0).abs() < 1e-5, "{t:?}");
            assert!(dot(xyz, *n).abs() < 1e-5, "{t:?} {n:?}");
            assert!(t[3] == 1.0 || t[3] == -1.0);
        }
    }

    #[test]
    fn uv_mapped_quad() {
        let mut q = quad();
        q.tangents.clear();
        q.ensure_tangents(0).unwrap();
        for t in &q.tangents {
            assert!(close(t, &[1.0, 0.0, 0.0, 1.0], 1e-6), "{t:?}");
        }
    }

    #[test]
    fn mirrored_quad_has_negative_handedness() {
        let mut q = mirrored_quad();
        q.generate_tangents(0).unwrap();
        for t in &q.tangents {
            assert!(close(t, &[-1.0, 0.0, 0.0, -1.0], 1e-6), "{t:?}");
        }
    }

    #[test]
    fn generates_missing_normals_first() {
        let mut q = quad();
        q.normals.clear();
        q.generate_tangents(0).unwrap();
        assert_eq!(q.normals.len(), 4);
        assert_unit_and_perpendicular(&q);
    }

    #[test]
    fn missing_uv_set() {
        let mut q = quad();
        let e = q.generate_tangents(2).unwrap_err();
        assert_eq!(e.kind, MeshErrorKind::MissingUvSet { set: 2, available: 1 });
    }

    #[test]
    fn degenerate_triangles_still_give_valid_tangents() {
        let mut p = quad();
        // all UVs equal, a zero-area triangle and an out-of-range index
        p.uv_sets[0] = vec![[0.5, 0.5]; 4];
        p.positions.push([0.0; 3]);
        p.normals.push([0.0, 1.0, 0.0]);
        p.uv_sets[0].push([0.0, 0.0]);
        p.triangles.push([4, 4, 4]);
        p.triangles.push([0, 1, 99]);
        p.generate_tangents(0).unwrap();
        assert_eq!(p.tangents.len(), 5);
        assert_unit_and_perpendicular(&p);
    }

    #[test]
    fn tangent_from_v_when_u_is_degenerate() {
        // no u direction, v runs along -Y: the tangent is derived from cross(up, normal)
        let t = vertex_tangent([0.0, 0.0, 1.0], [0.0; 3], [0.0, -1.0, 0.0]);
        assert!(close(&t, &[1.0, 0.0, 0.0, 1.0], 1e-6));
    }
}
