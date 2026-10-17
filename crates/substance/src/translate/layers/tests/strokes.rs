//! Painter's strokes become stroke records next to the (empty) paint pixels, wherever the brush work lands.

use super::fixtures::*;
use crate::model::LayerKind as PainterKind;
use crate::model::*;
use crate::translate::layers::strokes::{StrokeCount, pressure_at, recorded, report_strokes};
use crate::translate::report::Reporter;
use materialcraft::paint::stroke::{StrokeOwner, StrokeTool, ViewKind};
use materialcraft::paint::{Document, EffectKind, Fidelity, LayerKind, TextureSet};

fn painter_stroke(points: usize, view: PainterView, pressure: Vec<TimelineKey>) -> PainterStroke {
    PainterStroke {
        uid: None,
        points: (0..points)
            .map(|i| StrokeKey {
                position: [i as f32 * 0.1, 0.5],
                time: i as f32 * 0.1,
            })
            .collect(),
        pressure,
        tilt_keys: 0,
        complete: true,
        view_matrix: std::array::from_fn(|i| i as f32),
        projection_matrix: [0.5; 16],
        view,
        duplicate_count: 1,
        duplicate_matrix: [-1.0; 16],
        random_seed: 99,
    }
}

fn paint(tool: PaintTool, strokes: Vec<PainterStroke>, path_strokes: usize) -> PainterEffect {
    PainterEffect::Paint(PaintEffect {
        header: header(ALL, vec![]),
        tool,
        opacity: 1.0,
        strokes,
        path_strokes,
        brush_uid: Some(2978),
        colors: vec![],
        alpha: vec![],
        stencil: None,
    })
}

fn group(effects: Vec<PainterEffect>) -> PainterEffect {
    PainterEffect::Group(EffectGroup {
        header: header(ALL, vec![blend(ALL, 25, 1.0)]),
        effects,
    })
}

fn key(time: f32, value: f32) -> TimelineKey {
    TimelineKey { time, value }
}

/// A paint layer: its brush group (two tools), a second group, a bare paint effect, and a painted mask.
fn painted_layer() -> PainterLayer {
    let brush = paint(
        PaintTool::Brush,
        vec![painter_stroke(3, PainterView::Mesh, vec![key(0.0, 0.2), key(0.2, 0.6)])],
        0,
    );
    let eraser = paint(PaintTool::Eraser, vec![painter_stroke(2, PainterView::Uv, vec![])], 1);
    let smudge = paint(PaintTool::Smudge, vec![painter_stroke(4, PainterView::Mesh, vec![])], 0);
    let projection = paint(
        PaintTool::Projection,
        vec![painter_stroke(1, PainterView::Uv, vec![])],
        0,
    );
    let mut l = layer(
        1,
        "Paint",
        vec![group(vec![brush, eraser]), group(vec![smudge]), projection.clone()],
        vec![],
    );
    l.kind = PainterKind::Paint;
    l.mask = Some(PainterMask {
        enabled: true,
        initial: MaskInitial::Black,
        effects: vec![group(vec![projection.clone(), projection])],
    });
    l
}

#[test]
fn brush_work_becomes_stroke_records() {
    let stack = import(vec![painted_layer()]);
    let l = &stack.layers[0];
    let owners: Vec<StrokeOwner> = l.stroke_records().into_iter().map(|(o, _)| o).collect();
    assert_eq!(
        owners,
        [
            StrokeOwner::Paint,
            StrokeOwner::Effect { effect: 0 },
            StrokeOwner::Effect { effect: 1 },
            StrokeOwner::Mask { effect: 1 },
            StrokeOwner::Mask { effect: 2 },
        ]
    );
    assert!(matches!(&l.kind, LayerKind::Paint(p) if p.images.is_empty()));
    let LayerKind::Paint(base) = &l.kind else {
        unreachable!()
    };
    let base = base.strokes.as_ref().and_then(|s| s.record()).unwrap();
    assert_eq!(base.strokes.len(), 2, "both tools of the brush group");
    let (brush, eraser) = (&base.strokes[0], &base.strokes[1]);
    assert_eq!(
        (brush.tool, brush.view, eraser.tool, eraser.view),
        (StrokeTool::Paint, ViewKind::Mesh, StrokeTool::Eraser, ViewKind::Uv)
    );
    assert_eq!(brush.preset, "substance:DataBrush/2978");
    assert_eq!(
        (brush.seed, brush.symmetry.copies, brush.symmetry.transform[0]),
        (99, 1, -1.0)
    );
    assert_eq!((brush.camera.view[12], brush.camera.projection[0]), (12.0, 0.5));
    let pressures: Vec<f32> = brush.samples.iter().map(|s| s.pressure).collect();
    assert!((pressures[1] - 0.4).abs() < 1e-6, "{pressures:?}");
    assert_eq!((pressures[0], pressures[2]), (0.2, 0.6));
    assert_eq!(brush.samples[2].position, [0.2, 0.5]);
    assert_eq!(brush.samples[2].time, 0.2);
    assert!(
        eraser.samples.iter().all(|s| s.pressure == 1.0 && s.tilt == [0.0; 2]),
        "no timeline: a mouse"
    );
    let tools = |owner| {
        let (_, data) = l.stroke_records().into_iter().find(|(o, _)| *o == owner).unwrap();
        data.record()
            .unwrap()
            .strokes
            .iter()
            .map(|s| s.tool)
            .collect::<Vec<_>>()
    };
    assert_eq!(tools(StrokeOwner::Effect { effect: 0 }), [StrokeTool::Smudge]);
    assert_eq!(tools(StrokeOwner::Effect { effect: 1 }), [StrokeTool::Projection]);
    let mask = l.mask.as_ref().unwrap();
    assert!(
        matches!(&mask.effects[0].kind, EffectKind::Paint(p) if p.strokes.is_none()),
        "the mask's group has no strokes of its own"
    );
    assert_eq!(recorded(&stack.layers), (6, 3 + 2 + 4 + 1 + 1 + 1, 5));
}

#[test]
fn records_survive_the_document_format() {
    let stack = import(vec![painted_layer()]);
    let mut set = TextureSet::new("t", "T", SIZE);
    set.channels = channels().specs().to_vec();
    set.layers = stack.layers;
    let doc = Document {
        texture_sets: vec![set],
        ..Document::default()
    };
    let mut bytes = Vec::new();
    materialcraft::paint::format::save(&doc, &mut bytes).unwrap();
    let back = materialcraft::paint::format::load(std::io::Cursor::new(bytes.clone())).unwrap();
    assert_eq!(back, doc);
    assert_eq!(materialcraft::paint::stroke::totals(&back), (6, 12));
    let mut again = Vec::new();
    materialcraft::paint::format::save(&back, &mut again).unwrap();
    assert_eq!(again, bytes);
}

#[test]
fn pressure_follows_the_timeline() {
    let keys = [key(0.1, 0.2), key(0.3, 0.6), key(0.3, 0.9), key(0.5, 0.1)];
    assert_eq!(pressure_at(&[], 0.4), 1.0);
    assert_eq!(pressure_at(&keys, 0.0), 0.2, "held before the first key");
    assert!((pressure_at(&keys, 0.2) - 0.4).abs() < 1e-6);
    assert_eq!(pressure_at(&keys, 0.3), 0.9, "the last of equal times");
    assert!((pressure_at(&keys, 0.4) - 0.5).abs() < 1e-6);
    assert_eq!(pressure_at(&keys, 9.0), 0.1, "held after the last key");
}

#[test]
fn the_report_counts_strokes_per_set() {
    let l = painted_layer();
    let painter = set(vec![l.clone()]);
    let count = StrokeCount::of_set(&painter);
    assert_eq!(
        count,
        StrokeCount {
            strokes: 6,
            samples: 12,
            with_pressure: 1,
            path_strokes: 1,
            incomplete: 0
        }
    );
    let stack = import(vec![l]);
    let mut reporter = Reporter::new("test".into());
    report_strokes(&mut reporter, "Set", &count, &stack.layers);
    report_strokes(&mut reporter, "Empty", &StrokeCount::default(), &[]);
    let report = reporter.finish();
    assert_eq!(report.entries.len(), 1);
    let e = &report.entries[0];
    assert_eq!(
        (e.item.as_str(), e.fidelity),
        ("texture set \"Set\" / strokes", Fidelity::Approximated)
    );
    assert!(
        e.note
            .starts_with("6 strokes (12 samples, 1 with pen pressure) kept as stroke records in 5 paint effects"),
        "{}",
        e.note
    );
    assert!(
        e.note.ends_with("; 1 3D path strokes kept raw in document.bin"),
        "{}",
        e.note
    );
}
