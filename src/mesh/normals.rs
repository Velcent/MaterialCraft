//! Vertex normal generation for parts that came without normals.

use super::{MeshPart, Vec3};
use crate::v3::{add, cross, dot, mul, sub};
use std::collections::HashMap;

/// Used for vertices that touch no triangle with area.
const FALLBACK_NORMAL: Vec3 = [0.0, 0.0, 1.0];

impl MeshPart {
    /// Replaces `normals` with smooth, area-weighted vertex normals. Vertices at exactly the same position
    /// (split at UV seams) share one normal so seams do not show. Triangles with bad indices are skipped.
    pub fn generate_normals(&mut self) {
        self.normals = smooth_normals(&self.positions, &self.triangles);
    }

    /// Generates normals only when the part has none.
    pub fn ensure_normals(&mut self) {
        if self.normals.is_empty() {
            self.generate_normals();
        }
    }
}

/// Area-weighted normals, accumulated per distinct position.
pub(super) fn smooth_normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let mut group_of_position: HashMap<[u32; 3], usize> = HashMap::new();
    let groups: Vec<usize> = positions
        .iter()
        .map(|p| {
            let next = group_of_position.len();
            *group_of_position.entry(position_key(*p)).or_insert(next)
        })
        .collect();
    let mut sum = vec![[0.0f32; 3]; group_of_position.len()];
    for tri in triangles {
        let i = tri.map(|i| i as usize);
        let (Some(&a), Some(&b), Some(&c)) = (positions.get(i[0]), positions.get(i[1]), positions.get(i[2])) else {
            continue;
        };
        // the cross product's length is twice the area, so summing it weights by area
        let face = cross(sub(b, a), sub(c, a));
        if !face.iter().all(|x| x.is_finite()) {
            continue;
        }
        for &v in &i {
            sum[groups[v]] = add(sum[groups[v]], face);
        }
    }
    groups.iter().map(|&g| unit_or(sum[g], FALLBACK_NORMAL)).collect()
}

/// Bit pattern of a position, with -0.0 folded into 0.0 so they weld.
pub(super) fn position_key(p: Vec3) -> [u32; 3] {
    p.map(|x| if x == 0.0 { 0 } else { x.to_bits() })
}

/// `v` normalised, or `fallback` when `v` is too short to have a direction.
pub(super) fn unit_or(v: Vec3, fallback: Vec3) -> Vec3 {
    let l2 = dot(v, v);
    if l2 > 1e-30 && l2.is_finite() { mul(v, 1.0 / l2.sqrt()) } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::test_util::{close, quad};

    #[test]
    fn flat_quad_faces_plus_z() {
        let mut q = quad();
        q.normals.clear();
        q.ensure_normals();
        assert_eq!(q.normals.len(), 4);
        for n in &q.normals {
            assert!(close(n, &[0.0, 0.0, 1.0], 1e-6));
        }
    }

    #[test]
    fn seam_vertices_share_a_normal() {
        // two faces of a roof meeting at a ridge, ridge vertices duplicated as at a UV seam
        let positions =
            vec![[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [2.0, 0.0, 0.0], [2.0, 0.0, 1.0]];
        let triangles = vec![[0, 1, 2], [2, 1, 3], [4, 5, 7], [4, 7, 6]];
        let n = smooth_normals(&positions, &triangles);
        assert!(close(&n[2], &n[4], 1e-6));
        assert!(close(&n[2], &[0.0, 1.0, 0.0], 1e-5));
    }

    #[test]
    fn degenerate_and_unused_vertices_get_a_fallback() {
        let positions = vec![[0.0; 3], [0.0; 3], [0.0; 3], [5.0, 5.0, 5.0]];
        let n = smooth_normals(&positions, &[[0, 1, 2], [0, 1, 9]]);
        assert!(n.iter().all(|v| *v == FALLBACK_NORMAL));
    }
}
