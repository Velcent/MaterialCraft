//! [`ActiveStroke`]: the stroke under the pointer, from pen down to pen up: the brush crate's
//! [`DocumentStroke`] plus what it paints through (the set's surface, the camera at pen down).

use super::super::document_state::OpenDocument;
use super::super::view_pick::ViewPicking;
use super::surfaces::Surface;
use crate::gui_paint::input::PointerSample;
use materialcraft::camera::Camera;
use materialcraft::mesh::Sphere;
use materialcraft::paint::Document;
use materialcraft::paint::brush::BrushPreset;
use materialcraft::paint::stroke::{StrokeTool, ViewKind};
use materialcraft::projection::ViewProjector;
use materialcraft::stamp::SymmetrySpec;
use materialcraft_brush::{BrushError, DocumentStroke, Ended, InputPoint, PaintTarget, PaintValues, SessionScene, StrokeSetup};
use std::sync::Arc;

/// How to start a stroke.
pub struct StrokeStart {
    pub target: PaintTarget,
    pub set: usize,
    pub preset: BrushPreset,
    pub tool: StrokeTool,
    pub values: PaintValues,
    pub occlusion: bool,
    pub seed: u32,
    pub camera: Camera,
    pub surface: Arc<Surface>,
}

pub struct ActiveStroke {
    stroke: DocumentStroke,
    set: usize,
    camera: Camera,
    surface: Arc<Surface>,
    /// Pen-down time, so samples are recorded in seconds since the stroke started.
    t0: f64,
    pub label: &'static str,
}

/// The pieces of a document a stroke needs at once: the mesh to look through, the set to paint.
fn split(doc: &mut Document, set: usize) -> Result<(&materialcraft::mesh::Mesh, &mut materialcraft::paint::TextureSet), BrushError> {
    let Document { mesh, texture_sets, .. } = doc;
    let set = texture_sets.get_mut(set).ok_or(BrushError::NotPaintable("the texture set is gone"))?;
    Ok((mesh, set))
}

impl ActiveStroke {
    /// Pen down at `at`: prepares the target, starts the stroke and paints its first sample. On failure after
    /// the target was prepared, what was done is recorded so the history stays whole.
    pub fn begin(doc: &mut OpenDocument, picking: &ViewPicking, start: StrokeStart, at: PointerSample) -> Result<ActiveStroke, BrushError> {
        let label = if start.tool == StrokeTool::Eraser { "Erase" } else { "Paint" };
        let model = doc.doc().mesh.bounding_sphere().unwrap_or(Sphere { centre: [0.0; 3], radius: 1.0 });
        let setup = StrokeSetup {
            preset: start.preset,
            tool: start.tool,
            seed: start.seed,
            symmetry: SymmetrySpec::None,
            view: ViewKind::Mesh,
            values: start.values.clone(),
            model,
            occlusion: start.occlusion,
        };
        let prepared = DocumentStroke::prepare(doc.doc_for_strokes(), start.target, &start.values)?;
        let result = {
            let (mesh, set) = split(doc.doc_for_strokes(), start.set)?;
            let projector = picking.projector_on(mesh, start.camera).ok_or(BrushError::NoView)?;
            let scene =
                SessionScene { surface: &start.surface.map, index: &start.surface.index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
            prepared.start(setup, &scene, mesh, &|part| picking.is_shown(part), set)
        };
        let stroke = match result {
            Ok(s) => s,
            Err(failed) => {
                let (ended, e) = *failed;
                record(doc, ended, label);
                return Err(e);
            }
        };
        let mut active = ActiveStroke { stroke, set: start.set, camera: start.camera, surface: start.surface, t0: at.time, label };
        active.add(doc, picking, &[at])?;
        Ok(active)
    }

    /// The texture set being painted.
    pub fn set(&self) -> usize {
        self.set
    }

    /// Paints more samples; returns whether any pixel changed.
    pub fn add(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, samples: &[PointerSample]) -> Result<bool, BrushError> {
        if samples.is_empty() {
            return Ok(false);
        }
        let points: Vec<InputPoint> =
            samples.iter().map(|s| InputPoint { pressure: s.pressure, ..InputPoint::at(s.pos, (s.time - self.t0).max(0.0)) }).collect();
        let (mesh, set) = split(doc.doc_for_strokes(), self.set)?;
        let projector = picking.projector_on(mesh, self.camera).ok_or(BrushError::NoView)?;
        let scene = SessionScene { surface: &self.surface.map, index: &self.surface.index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
        Ok(!self.stroke.add(&scene, &points, set)?.is_empty())
    }

    /// Pen up: paints the last stamps and records the stroke as one history step. Returns an error to report, if any.
    pub fn end(self, doc: &mut OpenDocument, picking: &ViewPicking) -> Option<String> {
        let ActiveStroke { stroke, set, camera, surface, label, .. } = self;
        let (ended, mut problem) = match split(doc.doc_for_strokes(), set) {
            Ok((mesh, set_ref)) => match picking.projector_on(mesh, camera) {
                Some(projector) => {
                    let scene = SessionScene { surface: &surface.map, index: &surface.index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
                    let (ended, result) = stroke.finish(&scene, set_ref);
                    (ended, result.err().map(|e| e.to_string()))
                }
                // the mesh changed under the stroke: keep what was painted, without its record
                None => (stroke.abandon(), Some("the 3D view changed during the stroke".to_string())),
            },
            Err(e) => (stroke.abandon(), Some(e.to_string())),
        };
        if let Some(e) = record(doc, ended, label) {
            problem.get_or_insert(e);
        }
        problem
    }
}

/// Records an ended stroke's inverses as one history step; returns a problem to report.
fn record(doc: &mut OpenDocument, ended: Ended, label: &str) -> Option<String> {
    let (inverses, problem) = ended.record(doc.doc_for_strokes());
    doc.record_stroke(label, inverses);
    problem.map(|e| format!("the stroke was painted but not added to the stroke record: {e}"))
}
