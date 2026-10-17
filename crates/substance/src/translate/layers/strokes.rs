//! Painter's brush strokes ([`PainterStroke`]) as our stroke records ([`StrokeRecord`]), and the import report
//! line that counts them. The pixels stay as imported (Painter's cache); nothing replays the records yet.
//!
//! Mapping (Painter → ours): `DataStrokeKey` position and time → sample position and time; the pressure
//! timeline (`DataTimelineKey` x/value) is sampled at each point's time, linearly between keys and held at the
//! ends (no keys: pressure 1, a mouse); `tilt` is never filled in the corpus → 0; `viewMatrix` /
//! `projectionMatrix` → the camera (same column-major order); `viewType` 0/1 → UV/3D view; `duplicateCount` /
//! `duplicateMatrix` → symmetry; `randomSeed` → seed; the tool's class → the tool; the `DataBrush` uid → the
//! preset reference `substance:DataBrush/<uid>` (its settings stay in `document.bin`, kept under `foreign/`).
//! 3D path strokes (`DataStroke3D`) have no record yet; they are counted.

use crate::model::{PaintEffect, PaintTool, PainterEffect, PainterStroke, PainterTextureSet, PainterView, TimelineKey};
use crate::translate::report::Reporter;
use materialcraft::paint::stroke::{
    Camera, Stroke, StrokeData, StrokeRecord, StrokeSample, StrokeTool, Symmetry, ViewKind,
};
use materialcraft::paint::{Fidelity, Layer};

/// The record of the strokes of `effects`, in order; `None` when they hold no (2D) stroke.
pub fn record<'a>(effects: impl IntoIterator<Item = &'a PaintEffect>) -> Option<StrokeData> {
    let mut strokes = Vec::new();
    for e in effects {
        strokes.extend(e.strokes.iter().map(|s| stroke(e, s)));
    }
    (!strokes.is_empty()).then_some(StrokeData::Record(StrokeRecord { strokes }))
}

/// The record of the paint effects directly in `effects` (the brush work of a Painter effect group).
pub fn group_record(effects: &[PainterEffect]) -> Option<StrokeData> {
    record(effects.iter().filter_map(|e| match e {
        PainterEffect::Paint(p) => Some(p),
        _ => None,
    }))
}

fn tool(t: PaintTool) -> StrokeTool {
    match t {
        PaintTool::Brush => StrokeTool::Paint,
        PaintTool::Projection => StrokeTool::Projection,
        PaintTool::Eraser => StrokeTool::Eraser,
        PaintTool::Smudge => StrokeTool::Smudge,
    }
}

fn stroke(effect: &PaintEffect, s: &PainterStroke) -> Stroke {
    let view = match s.view {
        PainterView::Uv => ViewKind::Uv,
        PainterView::Mesh | PainterView::Other(_) => ViewKind::Mesh,
    };
    Stroke {
        tool: tool(effect.tool),
        preset: effect
            .brush_uid
            .map_or(String::new(), |uid| format!("substance:DataBrush/{uid}")),
        view,
        camera: Camera {
            view: s.view_matrix,
            projection: s.projection_matrix,
        },
        seed: s.random_seed,
        symmetry: Symmetry {
            copies: s.duplicate_count,
            transform: s.duplicate_matrix,
            flip: [false; 2],
        },
        samples: s
            .points
            .iter()
            .map(|p| StrokeSample {
                position: p.position,
                pressure: pressure_at(&s.pressure, p.time),
                tilt: [0.0; 2],
                time: p.time,
            })
            .collect(),
    }
}

/// The pressure timeline at `time`: linear between keys, held before the first and after the last; 1 without
/// keys.
pub fn pressure_at(keys: &[TimelineKey], time: f32) -> f32 {
    let after = keys.partition_point(|k| k.time <= time);
    match (after.checked_sub(1).and_then(|i| keys.get(i)), keys.get(after)) {
        (None, None) => 1.0,
        (Some(k), None) | (None, Some(k)) => k.value,
        (Some(a), Some(b)) => {
            let span = b.time - a.time;
            match span > 0.0 {
                true => a.value + (b.value - a.value) * (time - a.time) / span,
                false => a.value,
            }
        }
    }
}

/// Strokes of a Painter texture set, over every layer, mask and effect group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StrokeCount {
    pub strokes: usize,
    pub samples: usize,
    /// Strokes with a pressure timeline.
    pub with_pressure: usize,
    /// `DataStroke3D`s (no record yet).
    pub path_strokes: usize,
    /// Strokes whose point lists could not be read completely.
    pub incomplete: usize,
}

impl StrokeCount {
    pub fn of_set(set: &PainterTextureSet) -> StrokeCount {
        let mut count = StrokeCount::default();
        for layer in set.all_layers() {
            count.effects(&layer.content);
            if let Some(mask) = &layer.mask {
                count.effects(&mask.effects);
            }
        }
        count
    }

    fn effects(&mut self, effects: &[PainterEffect]) {
        for e in effects {
            match e {
                PainterEffect::Paint(p) => {
                    self.strokes += p.strokes.len();
                    self.samples += p.strokes.iter().map(|s| s.points.len()).sum::<usize>();
                    self.with_pressure += p.strokes.iter().filter(|s| !s.pressure.is_empty()).count();
                    self.incomplete += p.strokes.iter().filter(|s| !s.complete).count();
                    self.path_strokes += p.path_strokes;
                }
                PainterEffect::Group(g) => self.effects(&g.effects),
                _ => {}
            }
        }
    }
}

/// Strokes and samples in the records of `layers` (also in folders), and how many records hold them.
pub fn recorded(layers: &[Layer]) -> (usize, usize, usize) {
    let mut out = (0, 0, 0);
    let mut stack: Vec<&Layer> = layers.iter().collect();
    while let Some(l) = stack.pop() {
        stack.extend(l.children());
        for r in l.stroke_records().into_iter().filter_map(|(_, d)| d.record()) {
            out.0 += r.strokes.len();
            out.1 += r.sample_count();
            out.2 += 1;
        }
    }
    out
}

/// One line per set with strokes: how many became stroke records.
pub fn report_strokes(reporter: &mut Reporter, set: &str, painter: &StrokeCount, layers: &[Layer]) {
    if painter.strokes + painter.path_strokes == 0 {
        return;
    }
    let (strokes, samples, records) = recorded(layers);
    let mut note = format!(
        "{strokes} strokes ({samples} samples, {} with pen pressure) kept as stroke records in {records} paint effects; \
         not replayed yet, the pixels come from Painter",
        painter.with_pressure
    );
    if strokes != painter.strokes {
        note.push_str(&format!(
            "; {} of Painter's {} strokes have no record",
            painter.strokes - strokes.min(painter.strokes),
            painter.strokes
        ));
    }
    if painter.incomplete > 0 {
        note.push_str(&format!(
            "; {} strokes with unreadable points (kept partly)",
            painter.incomplete
        ));
    }
    if painter.path_strokes > 0 {
        note.push_str(&format!(
            "; {} 3D path strokes kept raw in document.bin",
            painter.path_strokes
        ));
    }
    reporter.add(format!("texture set \"{set}\" / strokes"), Fidelity::Approximated, note);
}

/// The line for a set that is not imported (no mesh slot uses it) but holds strokes.
pub fn report_unimported_strokes(reporter: &mut Reporter, set: &str, painter: &StrokeCount) {
    if painter.strokes + painter.path_strokes == 0 {
        return;
    }
    reporter.add(
        format!("texture set \"{set}\" / strokes"),
        Fidelity::KeptRaw,
        format!(
            "{} strokes and {} 3D path strokes of a set that is not imported; kept raw in document.bin",
            painter.strokes, painter.path_strokes
        ),
    );
}
