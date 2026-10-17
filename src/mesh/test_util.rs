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

/// A 2 x 2 x 2 cube centred at the origin with every face's vertices split (24 vertices, flat normals), as importers
/// write it. UVs: the four side faces form one strip (u 0..1, v 1/3..2/3) whose wrap-around edge (u = 1 against
/// u = 0) is a seam; top and bottom are islands of their own in v 0.7..0.95. So: 8 welded vertices, 12 edges and
/// 3 UV islands.
pub fn seamed_cube() -> MeshPart {
    // columns around the Y axis, clockwise seen from above, so side faces wind counter-clockwise from outside
    let cols = [[-1.0f32, 1.0f32], [1.0, 1.0], [1.0, -1.0], [-1.0, -1.0]];
    let mut part = MeshPart { name: "cube".into(), uv_sets: vec![vec![]], ..MeshPart::default() };
    let mut face = |corners: [[f32; 3]; 4], uv: [[f32; 2]; 4]| {
        let base = part.positions.len() as u32;
        let n = crate::v3::norm(crate::v3::cross(crate::v3::sub(corners[1], corners[0]), crate::v3::sub(corners[2], corners[0])));
        part.positions.extend(corners);
        part.normals.extend([n; 4]);
        part.uv_sets[0].extend(uv);
        part.triangles.push([base, base + 1, base + 2]);
        part.triangles.push([base, base + 2, base + 3]);
    };
    for i in 0..4 {
        let (a, b) = (cols[i], cols[(i + 1) % 4]);
        let (u0, u1) = (i as f32 / 4.0, (i + 1) as f32 / 4.0);
        face(
            [[a[0], -1.0, a[1]], [b[0], -1.0, b[1]], [b[0], 1.0, b[1]], [a[0], 1.0, a[1]]],
            [[u0, 2.0 / 3.0], [u1, 2.0 / 3.0], [u1, 1.0 / 3.0], [u0, 1.0 / 3.0]],
        );
    }
    let cap_uv = |u0: f32| [[u0, 0.95], [u0 + 0.2, 0.95], [u0 + 0.2, 0.7], [u0, 0.7]];
    let top = cols.map(|c| [c[0], 1.0, c[1]]);
    face(top, cap_uv(0.1));
    let bottom = [cols[0], cols[3], cols[2], cols[1]].map(|c| [c[0], -1.0, c[1]]);
    face(bottom, cap_uv(0.6));
    part
}

pub fn close(a: &[f32], b: &[f32], eps: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
}
