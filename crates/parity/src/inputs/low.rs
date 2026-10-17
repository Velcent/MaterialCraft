//! Adapter from our [`Mesh`] (glTF conventions: UV origin top-left) to the baker's triangle soup
//! ([`bake::Mesh`], OBJ conventions: v up). The baker rasterises row `(1 − v) × size`, so flipping v here
//! makes its row 0 the top of the texture, as Painter's image resources are stored.

use materialcraft::bake::{self, Tri};
use materialcraft::mesh::Mesh;

/// Triangles of the parts on `slots` (all parts when `slots` is `None`), one baker group per part.
pub fn to_bake_mesh(mesh: &Mesh, slots: Option<&[usize]>) -> bake::Mesh {
    let mut out = bake::Mesh::default();
    for part in &mesh.parts {
        if slots.is_some_and(|s| !s.contains(&part.material)) {
            continue;
        }
        let group = out.groups.len() as u32;
        out.groups.push(part.name.clone());
        let uvs = part.uv_sets.first();
        for t in &part.triangles {
            let idx = [t[0] as usize, t[1] as usize, t[2] as usize];
            let (Some(a), Some(b), Some(c)) = (
                part.positions.get(idx[0]),
                part.positions.get(idx[1]),
                part.positions.get(idx[2]),
            ) else {
                continue;
            };
            let p = [*a, *b, *c];
            let face = face_normal(&p);
            let n = idx.map(|i| part.normals.get(i).copied().map_or(face, normalised));
            let uv = idx.map(|i| {
                let uv = uvs.and_then(|u| u.get(i)).copied().unwrap_or([0.0, 0.0]);
                [uv[0], 1.0 - uv[1]]
            });
            out.tris.push(Tri { p, n, uv, group });
        }
    }
    out
}

/// Whole-mesh bounds of a baker mesh: (min, max).
pub fn bounds(mesh: &bake::Mesh) -> Option<([f32; 3], [f32; 3])> {
    let mut points = mesh.tris.iter().flat_map(|t| t.p.iter());
    let first = *points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| {
        (
            [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
            [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
        )
    }))
}

fn face_normal(p: &[[f32; 3]; 3]) -> [f32; 3] {
    let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    normalised([
        e1[1] * e2[2] - e1[2] * e2[1],
        e1[2] * e2[0] - e1[0] * e2[2],
        e1[0] * e2[1] - e1[1] * e2[0],
    ])
}

fn normalised(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-20 && l.is_finite() {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 0.0, 1.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::mesh::MeshPart;

    fn part(material: usize) -> MeshPart {
        MeshPart {
            name: format!("p{material}"),
            material,
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            uv_sets: vec![vec![[0.0, 0.0], [1.0, 0.0], [0.0, 0.25]]],
            triangles: vec![[0, 1, 2], [0, 1, 9]],
            ..MeshPart::default()
        }
    }

    #[test]
    fn flips_v_filters_slots_and_skips_bad_indices() {
        let mesh = Mesh {
            materials: vec!["a".into(), "b".into()],
            parts: vec![part(0), part(1)],
        };
        let one = to_bake_mesh(&mesh, Some(&[1]));
        assert_eq!(
            (one.tris.len(), one.groups.as_slice()),
            (1, ["p1".to_string()].as_slice())
        );
        assert_eq!(one.tris[0].uv[2], [0.0, 0.75]);
        assert_eq!(one.tris[0].n[0], [0.0, 0.0, 1.0]);
        let all = to_bake_mesh(&mesh, None);
        assert_eq!((all.tris.len(), all.tris[1].group), (2, 1));
        assert_eq!(bounds(&all), Some(([0.0; 3], [1.0, 1.0, 0.0])));
        assert_eq!(bounds(&bake::Mesh::default()), None);
    }
}
