//! The gates of steps 38 and 43: gizmo drags, surface snaps, UV handle drags, warp grid point drags and panel slider
//! drags on an open document are one history step each, and undo returns the projection they started from. Headless: a 2 × 2 quad facing +z
//! seen straight on through an orthographic camera (100 × 100 view units over 2 object units, the centre at
//! (50, 50), y down), picked on the CPU.

use super::*;
use materialcraft::camera::{Camera, Lens};
use materialcraft::mesh::{Fit, Mesh, MeshPart, SurfaceKey};
use materialcraft::paint::edit::LayerRef;
use materialcraft::paint::edit::ops::FillAt;
use materialcraft::paint::{BlendMode, Channel, Document, Fill, Layer, LayerKind, TextureSet};
use materialcraft::projection::gizmo::Handle;
use materialcraft::projection::{ImagePlacement, ProjectionMode, ViewScene, Volume};
use std::path::PathBuf;

fn quad() -> MeshPart {
    MeshPart {
        name: "quad".into(),
        material: 0,
        positions: vec![[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0], [-1.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
        uv_sets: vec![vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        faces: Default::default(),
    }
}

fn open(mode: &str) -> OpenDocument {
    let mut set = TextureSet::new("s", "S", (64, 64)).with_channel(Channel::Roughness);
    let projection = Projection { mode: ProjectionMode::default_of(mode).unwrap(), ..Projection::default() };
    let fill = Fill::uniform(vec![(Channel::Roughness, [0.2; 4])]).with_projection(projection);
    set.layers.push(Layer::new("f", "F", LayerKind::Fill(fill)).with_blend(Channel::Roughness, BlendMode::Normal, 1.0));
    let mut doc = Document::new(Mesh { materials: vec!["m".into()], parts: vec![quad()] });
    doc.texture_sets.push(set);
    OpenDocument::new(doc, PathBuf::from("test.mcpaint"), None)
}

fn target() -> FillRef {
    FillRef::new(LayerRef::new("s", "f"), FillAt::Layer)
}

fn camera() -> Camera {
    Camera::looking_at([0.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, 0.0], &Lens::Orthographic { height: 2.0, near: 0.1, far: 10.0 }, [100.0, 100.0])
}

fn projection(doc: &OpenDocument) -> Projection {
    target().fill(doc.doc()).unwrap().projection.clone()
}

fn volume(doc: &OpenDocument) -> Volume {
    *projection(doc).mode.volume().unwrap()
}

fn at(at: [f32; 2], phase: Phase) -> GizmoPointer {
    GizmoPointer { at, phase, step: false, shift: false }
}

/// Runs a press, drags and a release; returns the history length before and after.
fn drag(editing: &mut ProjectionEditing, doc: &mut OpenDocument, f: &GizmoFrame, path: &[[f32; 2]], step: bool, shift: bool) -> (usize, usize) {
    let before = doc.history().len();
    let n = path.len();
    for (i, p) in path.iter().enumerate() {
        let phase = if i == 0 {
            Phase::Press
        } else if i + 1 == n {
            Phase::Release
        } else {
            Phase::Drag
        };
        editing.pointer(doc, &target(), f, GizmoPointer { step, shift, ..at(*p, phase) }).unwrap();
    }
    assert!(!editing.is_dragging());
    (before, doc.history().len())
}

#[test]
fn a_gizmo_drag_is_one_undo_step() {
    let mut doc = open("planar");
    let mesh = doc.doc().mesh.clone();
    let scene = ViewScene::new(&mesh, Fit::IDENTITY);
    let view = scene.projector(&mesh, camera()).unwrap();
    let frame = SceneFrame::of_mesh(&mesh);
    let f = GizmoFrame { view: &view, frame: frame.as_ref(), surface: None };
    let mut editing = ProjectionEditing::default();
    let start = projection(&doc);
    // the x arrow runs right from the centre; 90-point arms are 1.8 object units
    assert_eq!(editing.hover(&doc, &target(), &f, [95.0, 50.0]), Some(GizmoHit::Volume(Handle::MoveAxis(0))));
    let (before, after) = drag(&mut editing, &mut doc, &f, &[[95.0, 50.0], [100.0, 52.0], [110.0, 49.0], [120.0, 50.0], [120.0, 50.0]], false, false);
    assert_eq!(after, before + 1, "one step for the whole drag");
    // 25 view units = 0.5 object units = 0.25 scene units (the scene is 2 wide)
    assert!((volume(&doc).offset[0] - 0.25).abs() < 1e-4, "{:?}", volume(&doc));
    let moved = projection(&doc);
    doc.undo().unwrap();
    assert_eq!(projection(&doc), start, "one undo returns to before the drag");
    doc.redo().unwrap();
    assert_eq!(projection(&doc), moved);

    // a stepped turn about the volume's z on its ring (90 view units around (75, 50)): 50° becomes a quarter turn,
    // still one step
    editing.settings.mode = GizmoMode::Rotate;
    let ring = |deg: f32| [75.0 + 90.0 * deg.to_radians().cos(), 50.0 - 90.0 * deg.to_radians().sin()];
    assert_eq!(editing.hover(&doc, &target(), &f, ring(30.0)), Some(GizmoHit::Volume(Handle::RotateAxis(2))));
    let (before, after) = drag(&mut editing, &mut doc, &f, &[ring(30.0), ring(60.0), ring(80.0), ring(80.0)], true, false);
    assert_eq!(after, before + 1);
    let rot = volume(&doc).rotation;
    assert!((rot[2] - 90.0).abs() < 1e-3 && rot[0].abs() < 1e-3 && rot[1].abs() < 1e-3, "{rot:?}");
    doc.undo().unwrap();
    assert_eq!(projection(&doc), moved);

    // a press beside every handle grabs nothing and records nothing
    let (before, after) = drag(&mut editing, &mut doc, &f, &[[5.0, 95.0], [20.0, 80.0], [30.0, 70.0]], false, false);
    assert_eq!(after, before);
}

#[test]
fn snapping_onto_the_surface_is_one_step_and_faces_the_normal() {
    let mut doc = open("planar");
    let mesh = doc.doc().mesh.clone();
    let scene = ViewScene::new(&mesh, Fit::IDENTITY);
    let view = scene.projector(&mesh, camera()).unwrap();
    let frame = SceneFrame::of_mesh(&mesh);
    let f = GizmoFrame { view: &view, frame: frame.as_ref(), surface: None };
    let mut editing = ProjectionEditing::default();
    let turned = Projection {
        mode: ProjectionMode::Planar(materialcraft::projection::Planar {
            volume: Volume { rotation: [90.0, 0.0, 0.0], ..Volume::default() },
            ..Default::default()
        }),
        ..Projection::default()
    };
    editing.set_from_panel(&mut doc, &target(), turned, false).unwrap();
    let start = projection(&doc);
    // Shift on the centre knob drags the volume over the surface
    let (before, after) = drag(&mut editing, &mut doc, &f, &[[50.0, 50.0], [60.0, 45.0], [75.0, 40.0], [75.0, 40.0]], false, true);
    assert_eq!(after, before + 1);
    let v = volume(&doc);
    // the point under (75, 40) is (0.5, 0.2, 0) = (0.25, 0.1, 0) scene units; facing +z: unturned
    assert!((v.offset[0] - 0.25).abs() < 1e-3 && (v.offset[1] - 0.1).abs() < 1e-3 && v.offset[2].abs() < 1e-4, "{v:?}");
    assert!(v.rotation.iter().all(|a| a.abs() < 1e-3), "{v:?}");
    doc.undo().unwrap();
    assert_eq!(projection(&doc), start);
}

#[test]
fn a_uv_handle_drag_is_one_step() {
    let mut doc = open("uv");
    let mesh = doc.doc().mesh.clone();
    let scene = ViewScene::new(&mesh, Fit::IDENTITY);
    let view = scene.projector(&mesh, camera()).unwrap();
    let key = SurfaceKey { material: 0, uv_set: 0, width: 64, height: 64, gutter: 0, mesh_revision: 1 };
    let map = SurfaceMap::build(&mesh, key).unwrap();
    let f = GizmoFrame { view: &view, frame: None, surface: Some(&map) };
    let mut editing = ProjectionEditing::default();
    // the image frame covers the quad: its centre knob is the view's centre, its right edge knob the quad's edge
    assert_eq!(editing.hover(&doc, &target(), &f, [50.5, 50.5]), Some(GizmoHit::Uv(UvHandle::Move)));
    assert_eq!(editing.hover(&doc, &target(), &f, [30.0, 70.0]), Some(GizmoHit::Uv(UvHandle::Move)));
    let (before, after) = drag(&mut editing, &mut doc, &f, &[[50.5, 50.5], [55.0, 50.5], [60.5, 50.5], [60.5, 50.5]], false, false);
    assert_eq!(after, before + 1);
    // 10 view units = 0.2 object units = 0.1 UV along +u
    let image = projection(&doc).image;
    assert!((image.offset[0] - 0.1).abs() < 1e-3 && image.offset[1].abs() < 1e-3, "{image:?}");
    doc.undo().unwrap();
    assert_eq!(projection(&doc).image, ImagePlacement::default());
}

#[test]
fn a_panel_slider_drag_is_one_step() {
    let mut doc = open("tri_planar");
    let mut editing = ProjectionEditing::default();
    let start = projection(&doc);
    let before = doc.history().len();
    for i in 1..=20 {
        let mut p = projection(&doc);
        p.image.rotation = i as f32;
        editing.set_from_panel(&mut doc, &target(), p, true).unwrap();
    }
    editing.end_panel_gesture(Some(&mut doc));
    let mut p = projection(&doc);
    p.filter = materialcraft::projection::Filter::Nearest;
    editing.set_from_panel(&mut doc, &target(), p, false).unwrap();
    assert_eq!(doc.history().len(), before + 2, "the drag, then the filter");
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(projection(&doc), start);
}

#[test]
fn a_warp_point_drag_is_one_step() {
    let mut doc = open("warp");
    let mesh = doc.doc().mesh.clone();
    let scene = ViewScene::new(&mesh, Fit::IDENTITY);
    let view = scene.projector(&mesh, camera()).unwrap();
    let frame = SceneFrame::of_mesh(&mesh);
    let f = GizmoFrame { view: &view, frame: frame.as_ref(), surface: None };
    let mut editing = ProjectionEditing::default();
    let start = projection(&doc);
    // the volume gizmo first; with the grid points shown, the middle anchor sits at the view's centre
    assert_eq!(editing.hover(&doc, &target(), &f, [50.0, 50.0]), Some(GizmoHit::Volume(Handle::MoveView)));
    editing.settings.warp_points = true;
    assert_eq!(editing.hover(&doc, &target(), &f, [50.0, 50.0]), Some(GizmoHit::WarpPoint(4)));
    assert_eq!(editing.hover(&doc, &target(), &f, [100.0, 0.0]), Some(GizmoHit::WarpPoint(2)), "the top-right corner");
    let (before, after) = drag(&mut editing, &mut doc, &f, &[[50.0, 50.0], [53.0, 49.0], [57.0, 46.0], [60.0, 45.0], [60.0, 45.0]], false, false);
    assert_eq!(after, before + 1, "one step for the whole drag");
    // 10 view units right and 5 up = (0.2, 0.1) object units = grid units (the volume is the 2-unit scene)
    let ProjectionMode::Warp(w) = &projection(&doc).mode else { panic!() };
    let p = w.grid.point_position(4).unwrap();
    assert!((p[0] - 0.2).abs() < 1e-4 && (p[1] - 0.1).abs() < 1e-4 && p[2].abs() < 1e-4, "{p:?}");
    assert_eq!(w.grid.points.len(), 9);
    let moved = projection(&doc);
    doc.undo().unwrap();
    assert_eq!(projection(&doc), start, "one undo returns to the regular grid");
    doc.redo().unwrap();
    assert_eq!(projection(&doc), moved);
    // a press away from every point grabs nothing
    let (before, after) = drag(&mut editing, &mut doc, &f, &[[25.0, 75.0], [30.0, 70.0], [30.0, 70.0]], false, false);
    assert_eq!(after, before);
}
