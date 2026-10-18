//! Geometry masks: the composite equals masking each layer by the surface map's part ids.

use super::*;
use crate::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use crate::paint::{Fill, GeometryMask, Mask};

const R: Channel = Channel::Roughness;

/// The test quad split into two parts: "lower" (triangle 0, u + v > 1) and "upper" (triangle 1).
fn two_parts() -> Mesh {
    let quad = crate::mesh::test_util::quad();
    let part = |name: &str, tri: usize| MeshPart { name: name.into(), triangles: vec![quad.triangles[tri]], ..quad.clone() };
    Mesh { materials: vec!["m".into()], parts: vec![part("lower", 0), part("upper", 1)] }
}

fn surface(size: u32) -> SurfaceMap {
    SurfaceMap::build(&two_parts(), SurfaceKey { material: 0, uv_set: 0, width: size, height: size, gutter: 0, mesh_revision: 0 }).unwrap()
}

fn fill_layer(id: &str, v: f32) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(R, [v, v, v, 1.0])]))).with_blend(R, BlendMode::Normal, 1.0)
}

fn parts(names: &[&str]) -> Option<GeometryMask> {
    Some(GeometryMask::Parts(names.iter().map(|n| n.to_string()).collect()))
}

/// The set: a base of 0.2, a layer of 0.8 on "lower", a folder (normal, mask 0.5) holding 0.6 on "upper".
fn set() -> TextureSet {
    let mut s = TextureSet::new("s", "S", (8, 8)).with_channel(R);
    s.layers.push(fill_layer("base", 0.2));
    let mut lower = fill_layer("lower only", 0.8);
    lower.geometry_mask = parts(&["lower", "not in this mesh"]);
    s.layers.push(lower);
    let mut folder = Layer::new("folder", "Folder", LayerKind::Group(vec![fill_layer("inner", 0.6)])).with_blend(R, BlendMode::Normal, 1.0);
    folder.mask = Some(Mask::new(0.5));
    folder.geometry_mask = parts(&["upper"]);
    s.layers.push(folder);
    s
}

#[test]
fn the_composite_masks_each_layer_by_the_part_under_the_texel() {
    let (s, map) = (set(), surface(8));
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&map, None);
    let c = Compositor::new(&s).with_fills(&fills);
    assert!(c.warnings(R).is_empty(), "{:?}", c.warnings(R));
    let (tex, _) = c.composite_tex(R);
    let got: Vec<f32> = tex.px.iter().map(|p| p[0]).collect();
    let expected = expected_by_part(&map);
    assert!(close(&got, &expected), "{got:?}\n{expected:?}");
    assert!(expected.contains(&0.8) && expected.iter().any(|v| (v - 0.4).abs() < 1e-4), "both parts are in the set");
}

/// Per texel what [`set`] composites to when each layer is masked by the name of the part under the texel.
fn expected_by_part(map: &SurfaceMap) -> Vec<f32> {
    (0..map.width() * map.height())
        .map(|i| {
            let (part, _, _) = map.triangle_index(i).expect("the quad covers the set");
            match map.part_name(part) {
                Some("lower") => 0.8,
                Some("upper") => 0.2 + (0.6 - 0.2) * 0.5,
                _ => 0.2,
            }
        })
        .collect()
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
}

#[test]
fn without_a_surface_map_the_mask_is_not_applied_and_reported() {
    let mut s = set();
    let c = Compositor::new(&s);
    let w = c.warnings(R);
    assert!(w.contains(&CompositeWarning::GeometryMaskNeedsMesh { layer: "lower only".into() }), "{w:?}");
    assert!(w.contains(&CompositeWarning::GeometryMaskNeedsMesh { layer: "folder".into() }), "{w:?}");
    let (tex, _) = c.composite_tex(R);
    assert!((tex.px[0][0] - 0.7).abs() < 1e-4, "0.8, then the folder at 0.5 everywhere");
    assert!(s.needs_surface());
    // UV tiles wait for UDIMs
    s.layers[1].geometry_mask = Some(GeometryMask::UvTiles(vec![1002]));
    s.layers[2].geometry_mask = None;
    let map = surface(8);
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&map, None);
    let w = Compositor::new(&s).with_fills(&fills).warnings(R);
    assert_eq!(w, vec![CompositeWarning::UvTileMaskNotComputable { layer: "lower only".into() }]);
    assert!(!s.needs_surface());
}

#[test]
fn a_mask_without_any_part_of_the_set_leaves_the_layer_out() {
    let mut s = set();
    s.layers[1].geometry_mask = Some(GeometryMask::nothing());
    s.layers[2].geometry_mask = parts(&["elsewhere"]);
    let map = surface(8);
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&map, None);
    let (tex, _) = Compositor::new(&s).with_fills(&fills).composite_tex(R);
    assert!(tex.px.iter().all(|p| (p[0] - 0.2).abs() < 1e-4));
}

#[test]
fn a_mesh_re_import_keeps_the_mask_by_part_names() {
    use crate::paint::Document;
    use crate::paint::edit::Edit;
    let mut doc = Document { mesh: two_parts(), ..Document::default() };
    doc.texture_sets.push(set());
    let key = |rev| SurfaceKey { material: 0, uv_set: 0, width: 8, height: 8, gutter: 0, mesh_revision: rev };
    // the composite next to masking by the part names under the texels of the current mesh
    let composite = |doc: &Document, rev| {
        let map = SurfaceMap::build(&doc.mesh, key(rev)).unwrap();
        let fills = FillContext::with_sources(&NoFillSources).with_surface(&map, None);
        let (tex, _) = Compositor::new(&doc.texture_sets[0]).with_fills(&fills).composite_tex(R);
        (tex.px.iter().map(|p| p[0]).collect::<Vec<f32>>(), expected_by_part(&map))
    };
    let (before, expected) = composite(&doc, 0);
    assert!(close(&before, &expected));
    // The re-imported mesh has its parts in another order and a new part on another material slot.
    let mut mesh = two_parts();
    mesh.parts.reverse();
    let mut extra = mesh.parts[0].clone();
    (extra.name, extra.material) = ("decal".into(), 1);
    mesh.parts.insert(1, extra);
    mesh.materials.push("n".into());
    let inverse = Edit::SetMesh(Box::new(mesh)).apply(&mut doc).unwrap();
    assert_eq!(doc.texture_sets[0].layers[1].geometry_mask, parts(&["lower", "not in this mesh"]), "the mask is untouched");
    let (after, expected) = composite(&doc, 1);
    assert!(close(&after, &expected), "the same parts are masked by name, whatever their index");
    let changed = before.iter().zip(&after).filter(|(a, b)| (*a - *b).abs() > 1e-4).count();
    assert!(changed <= 8, "only texels on the shared diagonal may go to the other part: {changed}");
    inverse.apply(&mut doc).unwrap();
    assert_eq!(composite(&doc, 0).0, before);
}
