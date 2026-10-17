use super::*;
use crate::mesh::test_util::{close, mirrored_quad, quad, seamed_cube};
use crate::mesh::{Mesh, MeshPart};

fn mesh(parts: Vec<MeshPart>) -> Mesh {
    let materials = (0..parts.iter().map(|p| p.material + 1).max().unwrap_or(1)).map(|i| format!("set {i}")).collect();
    Mesh { materials, parts }
}

fn key(m: &Mesh, size: u32, gutter: u32) -> SurfaceKey {
    SurfaceKey { gutter, ..SurfaceKey::square(m, 0, size) }
}

/// The quad with its UVs squeezed into `lo..hi` on both axes.
fn quad_in(lo: f32, hi: f32) -> MeshPart {
    let mut q = quad();
    for uv in &mut q.uv_sets[0] {
        *uv = uv.map(|c| lo + c * (hi - lo));
    }
    q
}

#[test]
fn full_quad_maps_every_texel() {
    let m = mesh(vec![quad()]);
    let s = SurfaceMap::build(&m, key(&m, 8, 0)).unwrap();
    for y in 0..8 {
        for x in 0..8 {
            let p = s.sample(x, y).unwrap();
            let (u, v) = ((x as f32 + 0.5) / 8.0, (y as f32 + 0.5) / 8.0);
            assert!(close(&p.position, &[-1.0 + 2.0 * u, 1.0 - 2.0 * v, 0.0], 1e-5), "{x} {y} {:?}", p.position);
            assert!(close(&p.normal, &[0.0, 0.0, 1.0], 1e-6));
            assert!(close(&p.tangent, &[1.0, 0.0, 0.0, 1.0], 1e-6));
            assert_eq!((p.part, p.island, p.kind), (0, 0, TexelKind::Covered));
            assert!((p.coverage - 1.0).abs() < 1e-6);
            assert!((p.barycentric.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        }
    }
    assert_eq!(s.sample(8, 0), None);
    assert_eq!(s.island_count(), 1);
}

#[test]
fn gutter_extends_the_surface_and_stops() {
    // the quad covers texels 4..12 of 16; a 2-texel gutter reaches 2..14
    let m = mesh(vec![quad_in(0.25, 0.75)]);
    let s = SurfaceMap::build(&m, key(&m, 16, 2)).unwrap();
    let x_of = |x: usize| -1.0 + 2.0 * ((x as f32 + 0.5) / 16.0 - 0.25) / 0.5;
    for x in 0..16 {
        let p = s.sample(x, 8);
        match x {
            4..12 => assert_eq!(p.unwrap().kind, TexelKind::Covered),
            2..4 | 12..14 => {
                let p = p.unwrap();
                assert_eq!((p.kind, p.coverage), (TexelKind::Gutter, 0.0));
                // continues the surface linearly instead of repeating the edge texel
                assert!((p.position[0] - x_of(x)).abs() < 1e-5, "{x}: {:?}", p.position);
            }
            _ => assert_eq!(p, None, "{x}"),
        }
    }
    // corners of the gutter are filled diagonally too
    assert!(s.sample(2, 2).is_some());
    assert_eq!(s.islands()[8 * 16 + 3], Some(0));
    assert_eq!(s.islands()[8 * 16 + 1], None);
}

#[test]
fn partly_covered_texels_have_partial_coverage() {
    // the right edge at u = 11.5 / 16 cuts texel column 11 in half
    let m = mesh(vec![quad_in(0.25, 11.5 / 16.0)]);
    let s = SurfaceMap::build(&m, key(&m, 16, 0)).unwrap();
    let edge = s.sample(11, 6).unwrap();
    assert!((edge.coverage - 0.5).abs() < 1e-6, "{}", edge.coverage);
    assert_eq!(edge.kind, TexelKind::Covered);
    assert!((s.sample(10, 6).unwrap().coverage - 1.0).abs() < 1e-6);
}

#[test]
fn mirrored_uvs_flip_the_tangent() {
    let mut q = mirrored_quad();
    q.tangents.clear(); // the fixture keeps the unmirrored quad's tangents
    let m = mesh(vec![q]);
    let s = SurfaceMap::build(&m, key(&m, 4, 0)).unwrap();
    let p = s.sample(0, 0).unwrap();
    assert!(close(&p.tangent, &[-1.0, 0.0, 0.0, -1.0], 1e-6), "{:?}", p.tangent);
    // u runs along -X now: texel column 0 is on the +X side
    assert!(p.position[0] > 0.0);
}

#[test]
fn cube_islands_and_coverage() {
    let m = mesh(vec![seamed_cube()]);
    let s = SurfaceMap::build(&m, key(&m, 24, 0)).unwrap();
    assert_eq!(s.island_count(), 3);
    // strip rows 8..16, top cap u 0.1..0.3 / v 0.7..0.95
    let strip = s.sample(12, 12).unwrap();
    let top = s.sample(5, 20).unwrap();
    assert_ne!(strip.island, top.island);
    assert!((top.position[1] - 1.0).abs() < 1e-5 && close(&top.normal, &[0.0, 1.0, 0.0], 1e-5));
    assert!((strip.position.iter().map(|c| c.abs()).fold(0.0, f32::max) - 1.0).abs() < 1e-5, "on the cube's surface");
    let area: f32 = s.coverage().iter().sum();
    let expected = (1.0 / 3.0 + 2.0 * 0.2 * 0.25) * 576.0;
    assert!((area - expected).abs() < 1.0, "{area} vs {expected}");
}

#[test]
fn only_the_sets_parts_are_drawn() {
    let mut other = quad();
    other.material = 1;
    other.positions.iter_mut().for_each(|p| p[2] = 3.0);
    let m = mesh(vec![quad_in(0.0, 0.5), other]);
    let s1 = SurfaceMap::build(&m, SurfaceKey { material: 1, ..key(&m, 8, 0) }).unwrap();
    let p = s1.sample(7, 7).unwrap();
    assert_eq!((p.part, p.position[2]), (1, 3.0));
    let s0 = SurfaceMap::build(&m, key(&m, 8, 0)).unwrap();
    assert_eq!(s0.sample(7, 7), None);
}

#[test]
fn generates_missing_normals_and_tangents() {
    let mut q = quad();
    q.normals.clear();
    q.tangents.clear();
    let m = mesh(vec![q]);
    let p = SurfaceMap::build(&m, key(&m, 4, 0)).unwrap().sample(1, 1).unwrap();
    assert!(close(&p.normal, &[0.0, 0.0, 1.0], 1e-6));
    assert!(close(&p.tangent, &[1.0, 0.0, 0.0, 1.0], 1e-6));
}

#[test]
fn errors() {
    let m = mesh(vec![quad()]);
    assert_eq!(SurfaceMap::build(&m, key(&m, 0, 0)), Err(SurfaceError::BadSize { width: 0, height: 0 }));
    assert!(SurfaceMap::build(&m, SurfaceKey { width: MAX_SURFACE_SIZE + 1, ..key(&m, 4, 0) }).is_err());
    assert_eq!(SurfaceMap::build(&m, SurfaceKey { material: 5, ..key(&m, 4, 0) }), Err(SurfaceError::NoSuchMaterial { material: 5 }));
    assert_eq!(SurfaceMap::build(&m, SurfaceKey { uv_set: 1, ..key(&m, 4, 0) }), Err(SurfaceError::MissingUvSet { part: 0, set: 1 }));
    // bad triangles and degenerate UVs are skipped, not fatal
    let mut q = quad();
    q.triangles.push([0, 1, 99]);
    q.uv_sets[0].push([0.5, 0.5]);
    q.positions.push([0.0; 3]);
    q.normals.push([0.0, 0.0, 1.0]);
    q.tangents.push([1.0, 0.0, 0.0, 1.0]);
    q.triangles.push([4, 4, 4]);
    let m = mesh(vec![q]);
    assert!(SurfaceMap::build(&m, key(&m, 4, 2)).is_ok());
}

#[test]
fn key_follows_the_mesh() {
    let m = mesh(vec![quad()]);
    let mut moved = m.clone();
    moved.parts[0].positions[0][0] = 0.5;
    assert_eq!(mesh_fingerprint(&m, 0), mesh_fingerprint(&m.clone(), 0));
    assert_ne!(mesh_fingerprint(&m, 0), mesh_fingerprint(&moved, 0));
    // another set's change doesn't touch this set's key
    assert_ne!(mesh_fingerprint(&m, 0), mesh_fingerprint(&m, 1));
    let s = SurfaceMap::build(&m, SurfaceKey::square(&m, 0, 4)).unwrap();
    assert_eq!((s.width(), s.height(), s.key().gutter), (4, 4, 8));
}
