//! Small meshes shared by the unit tests.

use super::MeshPart;

/// A 2 x 2 quad in the XY plane facing +Z, centred at the origin. UV origin top-left: u runs along +X, v along -Y,
/// so one UV unit spans 2 object units on both axes.
pub fn quad() -> MeshPart {
    MeshPart {
        name: "quad".into(),
        material: 0,
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
        uv_sets: vec![vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
    }
}

/// The same quad with its UVs mirrored horizontally (u runs along -X).
pub fn mirrored_quad() -> MeshPart {
    let mut p = quad();
    for uv in &mut p.uv_sets[0] {
        uv[0] = 1.0 - uv[0];
    }
    p
}

pub fn close(a: &[f32], b: &[f32], eps: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
}
