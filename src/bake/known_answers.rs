//! Each baker on synthetic meshes whose answer is known without Painter: a flat plane is open and flat, a box's
//! edges are convex (seen from inside, concave), a slab's thickness grows with its depth, positions span the
//! bounding box, and a sphere's normals are its radial directions.

use super::*;
use crate::mesh::{self, MeshPart};
use crate::v3::*;

/// The faces of an axis-aligned box of half extents `h` centred at the origin, each split into `n` x `n` quads and
/// laid out in its own cell of a 3 x 2 UV atlas (with a margin), flat normals pointing out (in, with `inward`).
fn boxed(h: V, inward: bool, n: usize) -> MeshPart {
    let mut part = MeshPart { name: "box".into(), uv_sets: vec![Vec::new()], ..MeshPart::default() };
    let n = n.max(1);
    let mut cell = 0;
    for axis in 0..3 {
        for sign in [1.0f32, -1.0] {
            // u, v span the face with cross(u, v) = outward normal
            let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
            let mut normal = [0.0; 3];
            normal[axis] = sign;
            let mut u = [0.0; 3];
            u[ua] = 1.0;
            let mut v = [0.0; 3];
            v[va] = sign;
            let centre = mul(normal, h[axis]);
            let (cx, cy) = ((cell % 3) as f32 / 3.0, (cell / 3) as f32 / 2.0);
            let base = part.positions.len() as u32;
            for j in 0..=n {
                for i in 0..=n {
                    let (fu, fv) = (i as f32 / n as f32, j as f32 / n as f32);
                    let p = add(centre, add(mul(u, (fu * 2.0 - 1.0) * h[ua]), mul(v, (fv * 2.0 - 1.0) * h[va])));
                    part.positions.push(p);
                    part.normals.push(if inward { mul(normal, -1.0) } else { normal });
                    let uv = [cx + (0.02 + 0.96 * fu) / 3.0, cy + (0.02 + 0.96 * (1.0 - fv)) / 2.0];
                    if let Some(set) = part.uv_sets.get_mut(0) {
                        set.push(uv);
                    }
                }
            }
            let at = |i: usize, j: usize| base + (j * (n + 1) + i) as u32;
            for j in 0..n {
                for i in 0..n {
                    let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
                    if inward {
                        part.triangles.extend([[a, c, b], [a, d, c]]);
                    } else {
                        part.triangles.extend([[a, b, c], [a, c, d]]);
                    }
                }
            }
            cell += 1;
        }
    }
    part
}

fn one(part: MeshPart) -> mesh::Mesh {
    mesh::Mesh { materials: vec!["m".into()], parts: vec![part] }
}

fn bake_low_as_high(low: &mesh::Mesh, s: &BakeSettings) -> BakedMaps {
    let high = HighScene::from_mesh(low);
    match bake_set(low, &TextureSetRef::slot(0), &high, s, &CancelToken::new(), &NullSink) {
        Ok(maps) => maps,
        Err(e) => panic!("bake failed: {e}"),
    }
}

fn only(kinds: &[MapKind], size: usize) -> BakeSettings {
    BakeSettings { size, padding: 0, maps: MapSelection::of(kinds), ..BakeSettings::default() }
}

/// Texel (x, y) of `t` given as fractions of its size.
fn at(t: &Tex, x: f32, y: f32) -> [f32; 4] {
    let (i, j) = (((x * t.w as f32) as usize).min(t.w - 1), ((y * t.h as f32) as usize).min(t.h - 1));
    t.px.get(j * t.w + i).copied().unwrap_or_default()
}

/// Covered texels of a map (the hit mask says which texels lie on the set).
fn covered<'a>(maps: &'a BakedMaps, kind: MapKind) -> impl Iterator<Item = [f32; 4]> + 'a {
    let t = maps.get(kind);
    maps.hit_mask.iter().enumerate().filter(|(_, h)| **h > 0.99).filter_map(move |(i, _)| t.and_then(|t| t.px.get(i).copied()))
}

#[test]
fn flat_plane_is_open_and_flat() {
    let plane = MeshPart {
        name: "plane".into(),
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uv_sets: vec![vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        ..MeshPart::default()
    };
    let maps = bake_low_as_high(&one(plane), &only(&[MapKind::AmbientOcclusion, MapKind::Curvature], 32));
    assert!(maps.hit_mask.iter().any(|h| *h > 0.99));
    assert!(covered(&maps, MapKind::AmbientOcclusion).all(|p| p[0] > 0.999), "a lone plane is unoccluded");
    assert!(covered(&maps, MapKind::Curvature).all(|p| (p[0] - 0.5).abs() < 1e-3), "a plane has no curvature");
}

#[test]
fn box_edges_are_convex_outside_and_concave_inside() {
    for inward in [false, true] {
        // smooth vertex normals: the shading turns across the quads along the edges, as on a bevel
        let mut part = boxed([1.0; 3], inward, 8);
        part.generate_normals();
        let maps = bake_low_as_high(&one(part), &only(&[MapKind::Curvature], 192));
        let Some(t) = maps.get(MapKind::Curvature) else { panic!("no curvature map") };
        // the first cell (+x face): its centre and a texel next to its left edge
        let centre = at(t, 0.5 / 3.0, 0.25)[0];
        let edge = at(t, 0.06 / 3.0, 0.25)[0];
        assert!((centre - 0.5).abs() < 0.12, "face centre near flat: {centre}");
        if inward {
            assert!(edge < 0.4 && edge < centre, "inside a box the edges are concave: {edge} vs {centre}");
        } else {
            assert!(edge > 0.6 && edge > centre, "a box's edges are convex: {edge} vs {centre}");
        }
    }
}

#[test]
fn slab_thickness_grows_with_depth() {
    let thickness = |depth: f32| {
        let s = BakeSettings {
            thickness: ThicknessSettings { maximise_range: false, max_dist: 0.5, ..ThicknessSettings::default() },
            ..only(&[MapKind::Thickness], 96)
        };
        let maps = bake_low_as_high(&one(boxed([1.0, 1.0, depth * 0.5], false, 1)), &s);
        // the +z face: cell 4 of the atlas
        maps.get(MapKind::Thickness).map_or(0.0, |t| at(t, 1.5 / 3.0, 0.75)[0])
    };
    let (thin, thick) = (thickness(0.05), thickness(0.1));
    assert!(thin > 0.0 && thick < 1.0, "{thin} {thick}");
    let ratio = thick / thin;
    assert!((1.5..2.5).contains(&ratio), "twice the depth, about twice the thickness: {thin} -> {thick}");
}

#[test]
fn position_spans_the_bounding_box() {
    let maps = bake_low_as_high(&one(boxed([2.0, 1.0, 0.5], false, 1)), &only(&[MapKind::Position], 64));
    let values: Vec<[f32; 4]> = covered(&maps, MapKind::Position).collect();
    let range = |c: usize| values.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[c]), hi.max(p[c])));
    // uniform: the longest side (x, 4 units) spans 0..1, the others are centred within it
    let (x, y, z) = (range(0), range(1), range(2));
    assert!(x.0 < 0.02 && x.1 > 0.98, "{x:?}");
    assert!((y.0 - 0.25).abs() < 0.02 && (y.1 - 0.75).abs() < 0.02, "{y:?}");
    assert!((z.0 - 0.375).abs() < 0.02 && (z.1 - 0.625).abs() < 0.02, "{z:?}");
}

#[test]
fn sphere_normals_are_radial() {
    let low = parse_obj(&uv_sphere_obj(16, 12, 1.0, 0.0)).unwrap_or_default();
    let high = parse_obj(&uv_sphere_obj(96, 64, 1.0, 0.0)).unwrap_or_default();
    let s = only(&[MapKind::WorldNormal, MapKind::Position, MapKind::Normal], 64);
    let maps = bake_set(&low.to_mesh(), &TextureSetRef::slot(0), &high.to_high(), &s, &CancelToken::new(), &NullSink).unwrap_or_default();
    let (Some(world), Some(position), Some(tangent)) = (maps.get(MapKind::WorldNormal), maps.get(MapKind::Position), maps.get(MapKind::Normal)) else {
        panic!("maps missing")
    };
    let mut checked = 0;
    for (i, _) in maps.hit_mask.iter().enumerate().filter(|(_, h)| **h > 0.99) {
        let (Some(w), Some(p), Some(n)) = (world.px.get(i), position.px.get(i), tangent.px.get(i)) else { continue };
        // the position map is centred on the sphere: its offset from 0.5 is the radial direction
        let radial = norm([p[0] - 0.5, p[1] - 0.5, p[2] - 0.5]);
        let baked = norm([w[0] * 2.0 - 1.0, w[1] * 2.0 - 1.0, w[2] * 2.0 - 1.0]);
        assert!(dot(radial, baked) > 0.995, "texel {i}: {baked:?} vs {radial:?}");
        // and the low sphere's own frame already points that way: nearly flat in tangent space
        assert!(n[2] > 0.95, "texel {i}: {n:?}");
        checked += 1;
    }
    assert!(checked > 1000, "{checked}");
}

#[test]
fn ao_darkens_into_concave_corners() {
    // inside a box every face is occluded by the others, most near the edges
    let s = BakeSettings { ao_rays: 256, ao_dist: 1.0, ..only(&[MapKind::AmbientOcclusion], 96) };
    let maps = bake_low_as_high(&one(boxed([1.0; 3], true, 4)), &s);
    let Some(t) = maps.get(MapKind::AmbientOcclusion) else { panic!("no AO map") };
    let (centre, edge) = (at(t, 0.5 / 3.0, 0.25)[0], at(t, 0.06 / 3.0, 0.25)[0]);
    assert!(centre < 0.9 && edge < centre - 0.1, "centre {centre}, edge {edge}");
    // seen from outside, a box is open everywhere
    let maps = bake_low_as_high(&one(boxed([1.0; 3], false, 4)), &s);
    assert!(covered(&maps, MapKind::AmbientOcclusion).all(|p| p[0] > 0.99));
}

#[test]
fn ao_can_ignore_back_faces() {
    // a box inside a larger one whose faces point outwards: seen from the inner box they are back faces
    let inner = boxed([1.0; 3], false, 2);
    let mut outer = boxed([1.5; 3], false, 2);
    outer.material = 1;
    let high = HighScene::from_mesh(&mesh::Mesh { materials: vec!["in".into(), "out".into()], parts: vec![inner.clone(), outer] });
    let low = one(inner);
    let mean = |ignore: bool| {
        let mut s = BakeSettings { ao_dist: 1.0, ao_rays: 64, ..only(&[MapKind::AmbientOcclusion], 48) };
        s.ao.ignore_backfaces = ignore;
        let maps = match bake_set(&low, &TextureSetRef::slot(0), &high, &s, &CancelToken::new(), &NullSink) {
            Ok(m) => m,
            Err(e) => panic!("bake failed: {e}"),
        };
        let v: Vec<f32> = covered(&maps, MapKind::AmbientOcclusion).map(|p| p[0]).collect();
        v.iter().sum::<f32>() / v.len().max(1) as f32
    };
    let (counted, ignored) = (mean(false), mean(true));
    assert!(counted < 0.5, "the enclosure occludes: {counted}");
    assert!(ignored > 0.99, "its back faces let the rays through: {ignored}");
}
