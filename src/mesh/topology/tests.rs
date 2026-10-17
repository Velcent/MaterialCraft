use super::*;
use crate::mesh::test_util::{mirrored_quad, quad, seamed_cube};

#[test]
fn quad_is_one_island_with_four_border_edges() {
    let t = Topology::of_part(&quad(), 0);
    assert_eq!(t.welded_vertex_count(), 4);
    assert_eq!(t.edges().len(), 5);
    assert_eq!(t.islands().count(), 1);
    assert_eq!(t.pieces().count(), 1);
    // the diagonal is edge 2 of triangle 0 (corner 2 -> 0) and edge 0 of triangle 1 (corner 0 -> 2)
    assert_eq!(t.neighbour(0, 2), Some(1));
    assert_eq!(t.neighbour(1, 0), Some(0));
    assert_eq!(t.neighbour(0, 0), None);
    let outline = t.uv_edges().of_island(0);
    assert_eq!(outline.len(), 4);
    assert!(outline.iter().all(|e| e.kind == UvEdgeKind::Border));
    assert_eq!(t.uv_edges().of_island(1), &[]);
}

#[test]
fn mirrored_uvs_are_still_one_island() {
    let t = Topology::of_part(&mirrored_quad(), 0);
    assert_eq!(t.islands().count(), 1);
    assert_eq!(t.uv_edges().all().len(), 4);
}

#[test]
fn seamed_cube_welds_and_finds_its_islands() {
    let cube = seamed_cube();
    let t = Topology::of_part(&cube, 0);
    assert_eq!(t.welded_vertex_count(), 8);
    assert_eq!(t.edges().len(), 18, "12 cube edges and 6 face diagonals");
    assert!(t.edges().iter().all(|e| e.uses == 2), "closed surface");
    assert_eq!(t.pieces().count(), 1);
    assert_eq!(t.islands().count(), 3, "side strip, top, bottom");
    let side = t.islands().of(0).unwrap();
    assert!((0..8).all(|tri| t.islands().of(tri) == Some(side)));
    assert_ne!(t.islands().of(8), t.islands().of(10));
    // the strip's outline: 4 top + 4 bottom edges and the wrap-around seam seen from both of its ends
    let strip = t.uv_edges().of_island(side);
    assert_eq!(strip.len(), 10);
    assert!(strip.iter().all(|e| e.kind == UvEdgeKind::Seam));
    assert_eq!(t.uv_edges().of_island(1).len(), 4);
    assert_eq!(t.uv_edges().all().len(), 18);
    // across a seam the welded neighbour is still found
    assert!(t.neighbour(8, 0).is_some());
}

#[test]
fn without_uvs_islands_follow_pieces() {
    let mut cube = seamed_cube();
    cube.uv_sets.clear();
    let t = Topology::of_part(&cube, 0);
    assert_eq!(t.islands().count(), 1);
    assert!(t.uv_edges().all().is_empty());
}

#[test]
fn bad_triangles_are_left_out() {
    let mut q = quad();
    q.triangles.push([0, 1, 99]);
    q.triangles.push([0, 0, 1]);
    let t = Topology::of_part(&q, 0);
    assert_eq!(t.triangle_edges(2), None);
    assert_eq!(t.islands().of(2), None);
    // a degenerate triangle still has its one real edge
    assert_eq!(t.triangle_edges(3).unwrap().iter().flatten().count(), 2);
    assert_eq!(t.neighbour(7, 0), None);
    assert_eq!(t.welded_vertex(99), None);
}

#[test]
fn separate_pieces() {
    let mut two = quad();
    let mut other = quad();
    for p in &mut other.positions {
        p[2] = 5.0;
    }
    two.positions.extend(other.positions);
    two.normals.extend(other.normals);
    two.uv_sets[0].extend(other.uv_sets[0].clone());
    two.triangles.extend(other.triangles.iter().map(|t| t.map(|i| i + 4)));
    let t = Topology::of_part(&two, 0);
    assert_eq!(t.pieces().count(), 2);
    assert_eq!(t.islands().count(), 2);
    assert_eq!(t.pieces().triangles(1).collect::<Vec<_>>(), vec![2, 3]);
}
