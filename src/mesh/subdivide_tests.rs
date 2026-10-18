use super::*;
use crate::mesh::test_util::{quad, seamed_cube};
use std::collections::HashMap;

/// Welded edge → number of triangles using it.
fn edge_use(p: &MeshPart) -> HashMap<(u32, u32), usize> {
    let (class, _) = weld(&p.positions);
    let mut uses = HashMap::new();
    for t in &p.triangles {
        for k in 0..3 {
            let (a, b) = (class[t[k] as usize], class[t[(k + 1) % 3] as usize]);
            *uses.entry(key(a, b)).or_insert(0) += 1;
        }
    }
    uses
}

fn assert_watertight(p: &MeshPart) {
    let uses = edge_use(p);
    assert!(uses.values().all(|&n| n == 2), "{} welded edges are not shared by exactly two triangles", uses.values().filter(|&&n| n != 2).count());
    let (_, class_pos) = weld(&p.positions);
    // a closed genus-0 surface: V - E + F = 2
    assert_eq!(class_pos.len() as i64 - uses.len() as i64 + p.triangles.len() as i64, 2);
}

#[test]
fn a_closed_seamed_cube_stays_watertight_at_every_level() {
    let cube = seamed_cube();
    assert_watertight(&cube);
    for scheme in [Scheme::Linear, Scheme::Loop] {
        for levels in 1..=3 {
            let s = cube.subdivided(levels, scheme);
            assert!(s.validate().is_ok());
            assert_eq!(s.triangles.len(), cube.triangles.len() * 4usize.pow(levels));
            assert_watertight(&s);
        }
    }
}

#[test]
fn uvs_follow_the_face_varying_rule() {
    let cube = seamed_cube();
    let s = cube.subdivided(1, Scheme::Loop);
    let (n, uv0, uv) = (cube.positions.len(), &cube.uv_sets[0], &s.uv_sets[0]);
    assert_eq!(&uv[..n], &uv0[..], "the original vertices keep their UVs");
    // every child triangle's corners are the parent's corners or midpoints of the parent's UV edges
    for (t, parent) in cube.triangles.iter().enumerate() {
        let [a, b, c] = parent.map(|v| uv0[v as usize]);
        let mid = |p: [f32; 2], q: [f32; 2]| [(p[0] + q[0]) * 0.5, (p[1] + q[1]) * 0.5];
        let expected = [[a, mid(a, b), mid(c, a)], [mid(a, b), b, mid(b, c)], [mid(c, a), mid(b, c), c], [mid(a, b), mid(b, c), mid(c, a)]];
        for (k, child) in s.triangles[4 * t..4 * t + 4].iter().enumerate() {
            assert_eq!(child.map(|v| uv[v as usize]), expected[k], "triangle {t} child {k}");
        }
    }
    // a seam edge splits into two vertices: same position, different UVs
    let mut by_pos: HashMap<[u32; 3], Vec<[f32; 2]>> = HashMap::new();
    for (p, t) in s.positions.iter().zip(uv).skip(n) {
        by_pos.entry(p.map(f32::to_bits)).or_default().push(*t);
    }
    let seams = by_pos.values().filter(|uvs| uvs.len() > 1 && uvs.iter().any(|u| u != &uvs[0])).count();
    assert!(seams > 0, "the cube's seams give new vertices on both sides");
    // and no vertex is created twice inside an island: 24 + one per face-varying edge (6 quads: 5 edges each)
    assert_eq!(s.positions.len(), 24 + 6 * 5);
}

#[test]
fn linear_keeps_the_surface_and_loop_rounds_it() {
    let q = quad().subdivided(2, Scheme::Linear);
    assert!(q.positions.iter().all(|p| p[2] == 0.0 && p[0].abs() <= 1.0 && p[1].abs() <= 1.0));
    assert_eq!(q.triangles.len(), 32);
    // a flat open quad stays flat under Loop too; its boundary corners stay put (2 boundary neighbours: the
    // boundary rule moves them along the boundary curve, a corner of a square towards the inside)
    let ql = quad().subdivided(1, Scheme::Loop);
    assert!(ql.positions.iter().all(|p| p[2] == 0.0));
    let cube = seamed_cube();
    let lin = cube.subdivided(2, Scheme::Linear);
    let max_r = |p: &MeshPart| p.positions.iter().map(|v| len(*v)).fold(0.0, f32::max);
    let min_r = |p: &MeshPart| p.positions.iter().map(|v| len(*v)).fold(f32::MAX, f32::min);
    assert!((max_r(&lin) - 3f32.sqrt()).abs() < 1e-6, "the corners stay");
    let smooth = cube.subdivided(3, Scheme::Loop);
    assert!(max_r(&smooth) < 1.2 && min_r(&smooth) > 0.5, "Loop rounds the cube: radius {} … {}", min_r(&smooth), max_r(&smooth));
    // every original corner moved inwards
    assert!((0..24).all(|v| len(smooth.positions[v]) < 3f32.sqrt() - 0.1));
}

#[test]
fn attributes_and_faces_are_carried() {
    let mut q = quad();
    q.faces = FaceGroups::from_polygon_sizes([4], 2);
    let s = q.subdivided(1, Scheme::Linear);
    assert_eq!((s.normals.len(), s.tangents.len()), (s.positions.len(), s.positions.len()));
    assert!(s.normals.iter().all(|n| *n == [0.0, 0.0, 1.0]));
    assert!(s.tangents.iter().all(|t| *t == [1.0, 0.0, 0.0, 1.0]));
    assert_eq!((s.faces.count(), s.faces.triangle_count()), (1, 8), "the quad is still one face");
    let per_tri = quad().subdivided(1, Scheme::Linear);
    assert!(per_tri.faces.triangle_count() == 0 || per_tri.faces.is_per_triangle());
    assert_eq!(quad().subdivided(0, Scheme::Loop), quad());
    let mut broken = quad();
    broken.triangles.push([0, 1, 99]);
    assert_eq!(broken.subdivided(2, Scheme::Loop), broken, "invalid parts are left alone");
    assert!((quad().longest_edge() - 8f32.sqrt()).abs() < 1e-6);
}
