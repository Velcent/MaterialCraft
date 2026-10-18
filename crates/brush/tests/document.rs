//! Strokes into a document: one undo step, a stroke record, and replays that give the same bits.

use materialcraft::camera::{Camera, Lens};
use materialcraft::history::{CoalesceKey, PaintHistory};
use materialcraft::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use materialcraft::paint::brush::{BrushPreset, Jitter};
use materialcraft::paint::edit::ops::{MaskStart, NewLayer, add_mask, create};
use materialcraft::paint::edit::{LayerRef, Transaction};
use materialcraft::paint::stroke::{StrokeData, StrokeTool, Symmetry, ViewKind};
use materialcraft::paint::{Channel, Document, LayerKind, TextureSet};
use materialcraft::projection::{ViewProjector, ViewScene};
use materialcraft::stamp::TexelIndex;
use materialcraft::symmetry::{Flip, SymmetrySpec};
use materialcraft_brush::symmetry::to_record;
use materialcraft_brush::{DocumentStroke, InputPoint, PaintTarget, PaintValues, ReplayScene, SessionScene, StrokeSetup, TargetPart, replay};

const SIZE: u32 = 256;

/// A UV sphere with a seam along one meridian (the built-in viewport sphere).
fn sphere() -> Mesh {
    let m = materialcraft::render3d::build_mesh(0, 48, 1.0, None, 0.0, 0.0);
    Mesh { materials: vec!["m".into()], parts: vec![MeshPart::from_viewport("sphere", 0, &m)] }
}

fn document() -> (Document, String) {
    let mut doc = Document { mesh: sphere(), ..Document::default() };
    doc.texture_sets.push(TextureSet::new("s", "S", (SIZE, SIZE)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness));
    let mut tx = Transaction::new(&mut doc);
    let id = create(&mut tx, "s", NewLayer::Paint, None).unwrap();
    tx.commit();
    (doc, id)
}

fn camera() -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    Camera::looking_at([0.4, 0.3, 3.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [480.0, 360.0])
}

fn values() -> PaintValues {
    PaintValues { channels: vec![(Channel::BaseColor, [0.9, 0.2, 0.1, 1.0], true), (Channel::Roughness, [0.3, 0.3, 0.3, 1.0], true)], mask: 1.0 }
}

fn setup(doc: &Document, tool: StrokeTool) -> StrokeSetup {
    let preset =
        BrushPreset { size: 0.2, flow: 0.6, opacity: 0.9, jitter: Jitter { size: 0.3, angle: 0.5, flow: 0.2, position: 0.2 }, ..BrushPreset::default() };
    StrokeSetup {
        preset,
        tool,
        seed: 11,
        symmetry: Symmetry::default(),
        view: ViewKind::Mesh,
        values: values(),
        model: doc.mesh.bounding_sphere().unwrap(),
        occlusion: true,
        retouch: Default::default(),
    }
}

/// Paints a wavy stroke across the sphere; returns the inverses for the history.
fn paint(
    doc: &mut Document,
    map: &SurfaceMap,
    index: &TexelIndex,
    view: &ViewScene,
    target: PaintTarget,
    tool: StrokeTool,
    symmetry: Symmetry,
) -> Vec<materialcraft::paint::edit::Edit> {
    let prepared = DocumentStroke::prepare(doc, target, &values()).unwrap();
    let Document { mesh, texture_sets, .. } = doc;
    let projector = view.projector(mesh, camera()).unwrap();
    let scene = SessionScene { surface: map, index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
    let set = &mut texture_sets[0];
    let mut stroke = prepared.start(StrokeSetup { symmetry, ..setup_for(mesh, tool) }, &scene, mesh, &|_| true, set).map_err(|b| b.1).unwrap();
    for i in 0..40 {
        let t = i as f32 / 39.0;
        let p = InputPoint { pressure: 0.5 + 0.5 * t, ..InputPoint::at([120.0 + 240.0 * t, 180.0 + 40.0 * (t * 6.0).sin()], f64::from(t)) };
        stroke.add(&scene, &[p], set).unwrap();
    }
    let (ended, result) = stroke.finish(&scene, set);
    result.unwrap();
    let (inverses, problem) = ended.record(doc);
    assert!(problem.is_none());
    inverses
}

fn setup_for(mesh: &Mesh, tool: StrokeTool) -> StrokeSetup {
    let doc = Document { mesh: mesh.clone(), ..Document::default() };
    setup(&doc, tool)
}

fn base(doc: &Document, layer: &str) -> Vec<u8> {
    match &doc.texture_sets[0].layer(layer).unwrap().kind {
        LayerKind::Paint(p) => p.image(Channel::BaseColor).unwrap().to_raw(),
        _ => panic!("not a paint layer"),
    }
}

#[test]
fn a_stroke_is_one_undo_step_with_a_record_and_replays_bit_for_bit() {
    let (mut doc, layer) = document();
    let map = SurfaceMap::build(&doc.mesh, SurfaceKey::square(&doc.mesh, 0, SIZE)).unwrap();
    let index = TexelIndex::build(&map);
    let view = ViewScene::new(&doc.mesh, doc.mesh.fit());
    let clean = doc.clone();
    let target = PaintTarget { layer: LayerRef::new("s", &layer), part: TargetPart::Content };

    let mut history = PaintHistory::default();
    let inverses = paint(&mut doc, &map, &index, &view, target.clone(), StrokeTool::Paint, Symmetry::default());
    history.record_all(&doc, "Paint", Some(CoalesceKey(1)), inverses);
    history.close();
    let painted = base(&doc, &layer);
    assert_ne!(painted, base(&clean, &layer), "the stroke painted something");
    let record = match &doc.texture_sets[0].layer(&layer).unwrap().kind {
        LayerKind::Paint(p) => p.strokes.clone(),
        _ => None,
    };
    let Some(StrokeData::Record(record)) = record else { panic!("no stroke record") };
    assert_eq!(record.strokes.len(), 1);
    assert_eq!(record.strokes[0].samples.len(), 40);

    // one undo step takes pixels and record back
    assert_eq!(history.len(), 1);
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, clean);
    history.redo(&mut doc).unwrap();
    assert_eq!(base(&doc, &layer), painted);

    // replaying the record twice gives the same bits, and the same as the live stroke within rounding of the
    // stored sample positions
    let replayed = |stroke: &materialcraft::paint::stroke::Stroke| {
        let mut d = clean.clone();
        let set = &mut d.texture_sets[0];
        let resolved = target.resolve(set, &materialcraft_brush::recorded_values(&materialcraft_brush::recorded_brush(stroke).unwrap())).unwrap();
        let mut targets = target.channel_targets(set, &resolved, materialcraft::stamp::StampMode::Paint).unwrap();
        let scene = ReplayScene { mesh: &clean.mesh, view: &view, surface: &map, index: &index, factors: &[] };
        replay(stroke, &scene, &mut targets).unwrap();
        drop(targets);
        base(&d, &layer)
    };
    let a = replayed(&record.strokes[0]);
    let b = replayed(&record.strokes[0]);
    assert_eq!(a, b);
    let differing = a.iter().zip(&painted).filter(|(x, y)| x != y).count();
    assert!(differing * 200 < a.len(), "replay differs from the live stroke in {differing} of {} bytes", a.len());
}

#[test]
fn masks_get_a_paint_effect_and_the_eraser_paints_them_black() {
    let (mut doc, layer) = document();
    let r = LayerRef::new("s", &layer);
    let mut tx = Transaction::new(&mut doc);
    add_mask(&mut tx, &r, MaskStart::White).unwrap();
    tx.commit();
    let map = SurfaceMap::build(&doc.mesh, SurfaceKey::square(&doc.mesh, 0, SIZE)).unwrap();
    let index = TexelIndex::build(&map);
    let view = ViewScene::new(&doc.mesh, doc.mesh.fit());
    let before = doc.clone();
    let target = PaintTarget { layer: r.clone(), part: TargetPart::Mask };
    let inverses = paint(&mut doc, &map, &index, &view, target.clone(), StrokeTool::Eraser, Symmetry::default());
    let mask = doc.texture_sets[0].layer(&layer).unwrap().mask.clone().unwrap();
    assert_eq!(mask.effects.len(), before.texture_sets[0].layer(&layer).unwrap().mask.as_ref().unwrap().effects.len() + 1);
    let Some(materialcraft::paint::EffectKind::Paint(px)) = mask.effects.last().map(|e| &e.kind) else { panic!("no paint effect") };
    let painted: Vec<[f32; 4]> = (0..SIZE).flat_map(|y| (0..SIZE).map(move |x| (x, y))).map(|(x, y)| px.image.get(x, y)).filter(|p| p[3] > 0.5).collect();
    assert!(!painted.is_empty() && painted.iter().all(|p| p[0] < 0.01), "the eraser paints the mask black");
    assert!(px.strokes.as_ref().and_then(|s| s.record()).is_some_and(|r| r.strokes[0].tool == StrokeTool::Eraser));
    let mut history = PaintHistory::default();
    history.record_all(&doc, "Erase", None, inverses);
    history.undo(&mut doc).unwrap();
    assert_eq!(doc, before);
}

/// Step 30's gate: a symmetric stroke records its symmetry, and replaying the record paints the same bits again
/// (the copies are built from the record's form live and on replay alike).
#[test]
fn symmetric_strokes_record_their_symmetry_and_replay_bit_for_bit() {
    for spec in [SymmetrySpec::Mirror { axis: 0, offset: 0.0 }, SymmetrySpec::Radial { axis: 1, point: [0.0; 3], count: 5, flip: Flip::U }] {
        let (mut doc, layer) = document();
        let map = SurfaceMap::build(&doc.mesh, SurfaceKey::square(&doc.mesh, 0, SIZE)).unwrap();
        let index = TexelIndex::build(&map);
        let view = ViewScene::new(&doc.mesh, doc.mesh.fit());
        let clean = doc.clone();
        let target = PaintTarget { layer: LayerRef::new("s", &layer), part: TargetPart::Content };
        paint(&mut doc, &map, &index, &view, target.clone(), StrokeTool::Paint, to_record(spec));
        let painted = base(&doc, &layer);
        let plain = {
            let mut d = clean.clone();
            paint(&mut d, &map, &index, &view, target.clone(), StrokeTool::Paint, Symmetry::default());
            base(&d, &layer)
        };
        let touched = |img: &[u8]| img.iter().filter(|b| **b != 0).count();
        assert!(touched(&painted) > touched(&plain) * 3 / 2, "{spec:?}: the copies painted more");
        let Some(StrokeData::Record(record)) = (match &doc.texture_sets[0].layer(&layer).unwrap().kind {
            LayerKind::Paint(p) => p.strokes.clone(),
            _ => None,
        }) else {
            panic!("no stroke record")
        };
        assert_eq!(record.strokes[0].symmetry, to_record(spec));
        let replayed = || {
            let mut d = clean.clone();
            let set = &mut d.texture_sets[0];
            let stroke = &record.strokes[0];
            let resolved = target.resolve(set, &materialcraft_brush::recorded_values(&materialcraft_brush::recorded_brush(stroke).unwrap())).unwrap();
            let mut targets = target.channel_targets(set, &resolved, materialcraft::stamp::StampMode::Paint).unwrap();
            let scene = ReplayScene { mesh: &clean.mesh, view: &view, surface: &map, index: &index, factors: &[] };
            replay(stroke, &scene, &mut targets).unwrap();
            drop(targets);
            base(&d, &layer)
        };
        let a = replayed();
        assert_eq!(a, replayed(), "{spec:?}");
        let differing = a.iter().zip(&painted).filter(|(x, y)| x != y).count();
        assert!(differing * 200 < a.len(), "{spec:?}: replay differs from the live stroke in {differing} of {} bytes", a.len());
    }
}
