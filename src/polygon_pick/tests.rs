use super::*;
use crate::camera::{Camera, Lens};
use crate::mesh::test_util::{quad, seamed_cube};
use crate::mesh::{FaceGroups, Mesh, MeshPart, SurfaceKey, SurfaceMap};
use crate::paint::blend::BlendMode;
use crate::paint::edit::ops::{PolygonFillChange, edit_polygon_fill};
use crate::paint::edit::{Edit, LayerRef, StackId, Transaction};
use crate::paint::effect::{Pick, PickMode, Selection};
use crate::paint::layer::Fill;
use crate::paint::{Channel, Compositor, Document, FillContext, Layer, LayerKind, Mask, NoFillSources, TextureSet};
use crate::projection::ViewScene;

const VIEW: [f32; 2] = [400.0, 300.0];

/// The seamed cube with its quads as source faces. Triangles: sides 0-7 (front +Z = 0,1; +X = 2,3; back -Z = 4,5;
/// -X = 6,7), top 8,9, bottom 10,11; islands: the side strip, top, bottom.
fn cube() -> Mesh {
    let mut part = seamed_cube();
    part.faces = FaceGroups::from_polygon_sizes([4; 6], 12);
    Mesh { materials: vec!["m".into()], parts: vec![part] }
}

/// Looking at the cube from the front, a little from the right and above: front, +X and top are visible.
fn camera() -> Camera {
    let (yaw, pitch) = (0.6f32, 0.4f32);
    let eye = [3.2 * yaw.sin() * pitch.cos(), 3.2 * pitch.sin(), 3.2 * yaw.cos() * pitch.cos()];
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    Camera::looking_at(eye, [0.0; 3], [0.0, 1.0, 0.0], &lens, VIEW)
}

fn tris(p: &Picked) -> Vec<u32> {
    p.get(&0).map(|s| s.iter().copied().collect()).unwrap_or_default()
}

fn pick(mesh: &Mesh, region: &PickRegion, visible_only: bool) -> Picked {
    let scene = ViewScene::new(mesh, mesh.fit());
    let view = scene.projector(mesh, camera()).unwrap();
    ScreenPick { projector: &view, mesh, material: 0, visible_only, shown: &|_| true }.pick(region)
}

/// Where the centre of the front face lands on screen.
fn front_centre(mesh: &Mesh) -> [f32; 2] {
    let scene = ViewScene::new(mesh, mesh.fit());
    let view = scene.projector(mesh, camera()).unwrap();
    use crate::projection::ViewProjector;
    view.to_screen([-0.3, 0.2, 1.0]).unwrap().pos
}

#[test]
fn a_click_grows_by_every_mode() {
    let mesh = cube();
    let topo = MeshTopologies::new(&mesh, 0);
    let clicked = pick(&mesh, &PickRegion::Point(front_centre(&mesh)), true);
    assert_eq!(tris(&clicked).len(), 1);
    assert!(tris(&clicked)[0] <= 1, "{clicked:?}");
    assert_eq!(tris(&topo.grow(&mesh, PickMode::Triangle, clicked.clone())).len(), 1);
    assert_eq!(tris(&topo.grow(&mesh, PickMode::Polygon, clicked.clone())), vec![0, 1]);
    assert_eq!(tris(&topo.grow(&mesh, PickMode::UvIsland, clicked.clone())), (0..8).collect::<Vec<_>>(), "the side strip, up to its seams");
    assert_eq!(tris(&topo.grow(&mesh, PickMode::Mesh, clicked.clone())), (0..12).collect::<Vec<_>>());
    // without source faces a face pick is the triangle pick (glTF meshes)
    let mut triangles_only = mesh.clone();
    triangles_only.parts[0].faces = FaceGroups::default();
    assert_eq!(MeshTopologies::new(&triangles_only, 0).grow(&triangles_only, PickMode::Polygon, clicked.clone()), clicked);
    // a click on nothing picks nothing
    assert!(pick(&mesh, &PickRegion::Point([1.0, 1.0]), true).is_empty());
}

#[test]
fn rectangles_and_lassos_pick_what_they_touch() {
    let mesh = cube();
    let all = PickRegion::Rect([0.0, 0.0], VIEW);
    assert_eq!(tris(&pick(&mesh, &all, false)), (0..12).collect::<Vec<_>>(), "through the model");
    assert_eq!(tris(&pick(&mesh, &all, true)), vec![0, 1, 2, 3, 8, 9], "only what the camera sees");
    // a small lasso around the front face's centre touches the front face only
    let c = front_centre(&mesh);
    let lasso = PickRegion::Lasso(vec![[c[0] - 3.0, c[1] - 3.0], [c[0] + 3.0, c[1] - 2.0], [c[0], c[1] + 4.0]]);
    let got = tris(&pick(&mesh, &lasso, true));
    assert!(!got.is_empty() && got.iter().all(|t| *t <= 1), "{got:?}");
    // a lasso that does not close an area picks nothing
    assert!(pick(&mesh, &PickRegion::Lasso(vec![c, c]), true).is_empty());
    // parts of another material slot are never picked
    let mut other = mesh.clone();
    other.parts[0].material = 1;
    other.materials.push("n".into());
    assert!(pick(&other, &all, false).is_empty());
}

#[test]
fn uv_picks_for_the_2d_view() {
    let mesh = cube();
    // the side strip's first face spans u 0..0.25, v 1/3..2/3
    assert_eq!(tris(&uv_pick(&mesh, 0, 0, &PickRegion::Point([0.2, 0.4]))).len(), 1);
    assert!(uv_pick(&mesh, 0, 0, &PickRegion::Point([0.9, 0.9])).is_empty());
    let strip = uv_pick(&mesh, 0, 0, &PickRegion::Rect([0.0, 0.4], [1.0, 0.6]));
    assert_eq!(tris(&strip), (0..8).collect::<Vec<_>>());
    assert!(uv_pick(&mesh, 0, 3, &PickRegion::Point([0.2, 0.4])).is_empty(), "no such UV set");
}

#[test]
fn picks_add_and_take_out_by_part_name() {
    let mesh = Mesh { materials: vec!["m".into()], parts: vec![quad(), MeshPart { name: "other".into(), ..quad() }] };
    let old = Selection { mode: PickMode::Triangle, picks: vec![Pick { part: "gone".into(), triangles: vec![4] }] };
    let picked: Picked = [(0, [1, 0].into_iter().collect()), (1, [1].into_iter().collect())].into_iter().collect();
    let added = apply_pick(&old, &mesh, &picked, true, PickMode::Polygon);
    assert_eq!(added.mode, PickMode::Polygon);
    assert_eq!(
        added.picks,
        vec![
            Pick { part: "gone".into(), triangles: vec![4] },
            Pick { part: "quad".into(), triangles: vec![0, 1] },
            Pick { part: "other".into(), triangles: vec![1] }
        ]
    );
    let taken: Picked = [(1, [1].into_iter().collect()), (0, [0].into_iter().collect())].into_iter().collect();
    let left = apply_pick(&added, &mesh, &taken, false, PickMode::Triangle);
    assert_eq!(left.picks, vec![Pick { part: "gone".into(), triangles: vec![4] }, Pick { part: "quad".into(), triangles: vec![1] }]);
}

// --- the gate: pick → resolution change → identical coverage at the new size ---

const R: Channel = Channel::Roughness;

fn document(size: u32) -> Document {
    let mut set = TextureSet::new("s", "S", (size, size)).with_channel(R);
    let fill = |id: &str, v: f32| Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(R, [v, v, v, 1.0])]))).with_blend(R, BlendMode::Normal, 1.0);
    set.layers.push(fill("below", 0.0));
    let mut top = fill("top", 1.0);
    top.mask = Some(Mask::new(0.0));
    set.layers.push(top);
    Document { mesh: cube(), texture_sets: vec![set], ..Document::default() }
}

/// The roughness the set composites to (= the top layer's mask), with the set's surface map at its resolution.
fn coverage(doc: &Document) -> Vec<f32> {
    let set = &doc.texture_sets[0];
    let key = SurfaceKey { material: 0, uv_set: 0, width: set.resolution.0, height: set.resolution.1, gutter: 0, mesh_revision: 0 };
    let surface = SurfaceMap::build(&doc.mesh, key).unwrap();
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&surface, None);
    let c = Compositor::new(set).with_fills(&fills);
    assert!(c.warnings(R).is_empty());
    c.composite_tex(R).0.px.iter().map(|p| p[0]).collect()
}

/// Rasterised independently: 1 where the texel's triangle is one of `tris`.
fn expected(doc: &Document, tris: &[u32]) -> Vec<f32> {
    let (w, h) = doc.texture_sets[0].resolution;
    let surface = SurfaceMap::build(&doc.mesh, SurfaceKey { material: 0, uv_set: 0, width: w, height: h, gutter: 0, mesh_revision: 0 }).unwrap();
    (0..(w * h) as usize).map(|i| if surface.triangle_index(i).is_some_and(|(p, t, _)| p == 0 && tris.contains(&(t as u32))) { 1.0 } else { 0.0 }).collect()
}

/// A click on the front face in UV-island mode, then the black key's un-pick of the front quad, each one step.
fn pick_into(doc: &mut Document) -> Vec<Edit> {
    let mesh = doc.mesh.clone();
    let topo = MeshTopologies::new(&mesh, 0);
    let layer = LayerRef::new("s", "top");
    let mut undo = Vec::new();
    for (mode, add) in [(PickMode::UvIsland, true), (PickMode::Polygon, false)] {
        let picked = topo.grow(&mesh, mode, pick(&mesh, &PickRegion::Point(front_centre(&mesh)), true));
        let mut tx = Transaction::new(doc);
        let change = edit_polygon_fill(&mut tx, &layer, StackId::Mask, |s| apply_pick(s, &mesh, &picked, add, mode)).unwrap();
        assert_ne!(change, PolygonFillChange::Unchanged);
        undo.push(tx.commit());
    }
    undo.into_iter().flatten().collect()
}

#[test]
fn picks_survive_a_resolution_change() {
    let mut doc = document(32);
    let undo = pick_into(&mut doc);
    assert_eq!(undo.len(), 2, "one edit per pick operation");
    let kept: Vec<u32> = (2..8).collect();
    assert_eq!(coverage(&doc), expected(&doc, &kept));
    assert!(coverage(&doc).contains(&1.0));
    // the set goes to 64 × 64 (images resampled, as a resolution change does)
    let mut bigger = doc.texture_sets[0].clone();
    bigger.resolution = (64, 64);
    let mut layers = std::mem::take(&mut bigger.layers);
    crate::paint::fragment::adapt::fit_layers(&mut layers, &bigger);
    bigger.layers = layers;
    Edit::ReplaceTextureSet(Box::new(bigger)).apply(&mut doc).unwrap();
    let at_64 = coverage(&doc);
    assert_eq!(at_64.len(), 64 * 64);
    assert_eq!(at_64, expected(&doc, &kept), "the picks rasterised afresh at the new size");
    // and the same as picking in a document made at that size
    let mut fresh = document(64);
    pick_into(&mut fresh);
    assert_eq!(at_64, coverage(&fresh));
    assert_eq!(fresh.texture_sets[0].layers[1].mask, doc.texture_sets[0].layers[1].mask);
}

#[test]
fn picks_undo_one_step_each_and_reach_content_too() {
    let mut doc = document(16);
    let before = doc.clone();
    let undo = pick_into(&mut doc);
    for inv in undo.into_iter().rev() {
        inv.apply(&mut doc).unwrap();
    }
    assert_eq!(doc, before);
    // a layer's content: a white polygon fill over every channel of the set
    let mesh = doc.mesh.clone();
    let picked: Picked = [(0, [0].into_iter().collect())].into_iter().collect();
    let mut tx = Transaction::new(&mut doc);
    let r = LayerRef::new("s", "below");
    let change = edit_polygon_fill(&mut tx, &r, StackId::Content, |s| apply_pick(s, &mesh, &picked, true, PickMode::Triangle)).unwrap();
    assert_eq!(change, PolygonFillChange::Added(0));
    // an un-pick of nothing selected changes nothing
    let none = edit_polygon_fill(&mut tx, &r, StackId::Content, |s| apply_pick(s, &mesh, &Picked::new(), false, PickMode::Triangle)).unwrap();
    assert_eq!(none, PolygonFillChange::Unchanged);
    tx.commit();
    assert_eq!(coverage(&doc), expected(&doc, &[0]), "content fill white on triangle 0, the masked top layer adds nothing");
}
