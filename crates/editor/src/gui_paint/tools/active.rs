//! [`ActiveStroke`]: the stroke under the pointer, from pen down to pen up: the brush crate's
//! [`DocumentStroke`] plus what it paints through (the set's surface, the camera at pen down), the lazy mouse in
//! front of it, and where it ended (the next straight line's start). Projection strokes (PJ-26) also keep where
//! the projected image lay at pen down and paint its values ([`Through::projection`]).

use super::super::document_state::OpenDocument;
use super::super::view_pick::ViewPicking;
use super::screen_image::ScreenImage;
use super::surfaces::Surface;
use crate::gui_paint::input::PointerSample;
use materialcraft::camera::Camera;
use materialcraft::mesh::Sphere;
use materialcraft::paint::Document;
use materialcraft::paint::brush::{BrushPreset, Retouch};
use materialcraft::paint::stroke::{StrokeTool, Symmetry, ViewKind};
use materialcraft::projection::{ScreenImageTransform, ViewProjector};
use materialcraft::stamp::{ProjectedPaint, TexelFactor};
use materialcraft_brush::{Base, BrushError, DocumentStroke, Ended, InputPoint, LazyMouse, LineAnchor, PaintTarget, PaintValues, SessionScene, StrokeSetup};
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
    /// Symmetric copies in the record's form.
    pub symmetry: Symmetry,
    /// The lazy mouse, if on (never for straight lines).
    pub lazy: Option<LazyMouse>,
    /// Smudge and blur strength, clone offset.
    pub retouch: Retouch,
    /// On a see-through layer, the composite below it (the stroke starts from it where the layer has no pixels).
    pub base: Option<Base>,
    /// Where the projected image lies (projection strokes; it does not move during a stroke).
    pub placement: Option<ScreenImageTransform>,
}

/// What a stroke paints through besides the surface, lent per call: the factors (quick mask, stencil) and, for
/// projection strokes, the projected image.
#[derive(Clone, Copy)]
pub struct Through<'a> {
    pub factors: &'a [&'a dyn TexelFactor],
    pub projection: Option<&'a ScreenImage>,
}

pub struct ActiveStroke {
    stroke: DocumentStroke,
    set: usize,
    camera: Camera,
    surface: Arc<Surface>,
    /// Pen-down time, so samples are recorded in seconds since the stroke started.
    t0: f64,
    pub label: &'static str,
    lazy: Option<LazyMouse>,
    /// The last point painted (view coordinates).
    last: Option<[f32; 2]>,
    placement: Option<ScreenImageTransform>,
}

/// How a stroke ended.
pub struct StrokeEnd {
    /// A problem to report.
    pub problem: Option<String>,
    /// Where it ended, for the next straight line.
    pub anchor: Option<LineAnchor>,
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
    pub fn begin(doc: &mut OpenDocument, picking: &ViewPicking, start: StrokeStart, at: PointerSample, through: Through) -> Result<ActiveStroke, BrushError> {
        let label = match start.tool {
            StrokeTool::Eraser => "Erase",
            StrokeTool::Smudge => "Smudge",
            StrokeTool::Blur => "Blur",
            StrokeTool::Clone => "Clone",
            StrokeTool::Projection => "Projection paint",
            StrokeTool::Paint => "Paint",
        };
        let model = doc.doc().mesh.bounding_sphere().unwrap_or(Sphere { centre: [0.0; 3], radius: 1.0 });
        let setup = StrokeSetup {
            preset: start.preset,
            tool: start.tool,
            seed: start.seed,
            symmetry: start.symmetry,
            view: ViewKind::Mesh,
            values: start.values.clone(),
            model,
            occlusion: start.occlusion,
            retouch: start.retouch,
        };
        let mut prepared = DocumentStroke::prepare(doc.doc_for_strokes(), start.target, &start.values)?;
        let result = {
            let (mesh, set) = split(doc.doc_for_strokes(), start.set)?;
            if let Some(base) = start.base {
                prepared.see_through(set, base);
            }
            let projector = picking.projector_on(mesh, start.camera).ok_or(BrushError::NoView)?;
            let projected = projected(through, start.placement, &start.surface, &projector);
            let scene = SessionScene {
                surface: &start.surface.map,
                index: &start.surface.index,
                projector: Some(&projector as &dyn ViewProjector),
                factors: through.factors,
                projected: projected.as_ref(),
            };
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
        let mut active = ActiveStroke {
            stroke,
            set: start.set,
            camera: start.camera,
            surface: start.surface,
            t0: at.time,
            label,
            lazy: start.lazy,
            last: None,
            placement: start.placement,
        };
        active.add(doc, picking, &[at], through)?;
        Ok(active)
    }

    /// True for a projection stroke (it paints through the projection image).
    pub fn projects(&self) -> bool {
        self.placement.is_some()
    }

    /// The texture set being painted.
    pub fn set(&self) -> usize {
        self.set
    }

    /// Paints more samples; returns whether any pixel changed.
    pub fn add(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, samples: &[PointerSample], through: Through) -> Result<bool, BrushError> {
        if samples.is_empty() {
            return Ok(false);
        }
        let mut points: Vec<InputPoint> = samples
            .iter()
            .map(|s| InputPoint { pressure: s.pressure, tilt: s.tilt, rotation: s.rotation, ..InputPoint::at(s.pos, (s.time - self.t0).max(0.0)) })
            .collect();
        if let Some(lazy) = &mut self.lazy {
            points = lazy.push(&points);
        }
        let Some(last) = points.last() else { return Ok(false) };
        self.last = Some(last.pos);
        let (mesh, set) = split(doc.doc_for_strokes(), self.set)?;
        let projector = picking.projector_on(mesh, self.camera).ok_or(BrushError::NoView)?;
        let projected = projected(through, self.placement, &self.surface, &projector);
        let scene = SessionScene {
            surface: &self.surface.map,
            index: &self.surface.index,
            projector: Some(&projector as &dyn ViewProjector),
            factors: through.factors,
            projected: projected.as_ref(),
        };
        Ok(!self.stroke.add(&scene, &points, set)?.is_empty())
    }

    /// The lazy mouse's string `(brush, pointer)` while the brush lags behind.
    pub fn string(&self) -> Option<([f32; 2], [f32; 2])> {
        self.lazy.as_ref().and_then(LazyMouse::string)
    }

    /// Pen up: paints the last stamps and records the stroke as one history step.
    pub fn end(self, doc: &mut OpenDocument, picking: &ViewPicking, through: Through) -> StrokeEnd {
        let ActiveStroke { stroke, set, camera, surface, label, last, placement, .. } = self;
        let mut anchor = None;
        let (ended, mut problem) = match split(doc.doc_for_strokes(), set) {
            Ok((mesh, set_ref)) => match picking.projector_on(mesh, camera) {
                Some(projector) => {
                    anchor = last.map(|end| LineAnchor::at(end, Some(&projector as &dyn ViewProjector)));
                    let projected = projected(through, placement, &surface, &projector);
                    let scene = SessionScene {
                        surface: &surface.map,
                        index: &surface.index,
                        projector: Some(&projector as &dyn ViewProjector),
                        factors: through.factors,
                        projected: projected.as_ref(),
                    };
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
        StrokeEnd { problem, anchor }
    }
}

/// The projection a projection stroke paints through `view` (none for other strokes).
fn projected<'a>(
    through: Through<'a>,
    placement: Option<ScreenImageTransform>,
    surface: &'a Surface,
    view: &'a dyn ViewProjector,
) -> Option<ProjectedPaint<'a>> {
    through.projection.zip(placement).map(|(image, p)| image.projected(&surface.map, view, p))
}

/// Records an ended stroke's inverses as one history step; returns a problem to report.
fn record(doc: &mut OpenDocument, ended: Ended, label: &str) -> Option<String> {
    let (inverses, problem) = ended.record(doc.doc_for_strokes());
    doc.record_stroke(label, inverses);
    problem.map(|e| format!("the stroke was painted but not added to the stroke record: {e}"))
}
