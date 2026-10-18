//! [`ActiveStroke`]: the stroke under the pointer, from pen down to pen up: the brush crate's
//! [`DocumentStroke`] plus what it paints through (the set's surface, the camera at pen down), the lazy mouse in
//! front of it, and where it ended (the next straight line's start). A stroke from the 2D view has no camera: its
//! points are texel positions and it paints in texture space (PT-53). Projection strokes (PJ-26) also keep where
//! the projected image lay at pen down and paint its values ([`Through::projection`]).

use super::super::document_state::OpenDocument;
use super::super::view_pick::ViewPicking;
use super::screen_image::ScreenImage;
use super::surfaces::Surface;
use crate::gui_paint::input::PointerSample;
use materialcraft::camera::Camera;
use materialcraft::mesh::Mesh;
use materialcraft::mesh::Sphere;
use materialcraft::paint::Document;
use materialcraft::paint::brush::{BrushPreset, Retouch};
use materialcraft::paint::stroke::{StrokeTool, Symmetry, ViewKind};
use materialcraft::projection::{CpuViewProjector, ScreenImageTransform, ViewProjector};
use materialcraft::stamp::{ProjectedPaint, TexelFactor};
use materialcraft_brush::{
    Base, BrushError, DocumentStroke, Ended, EngineExtras, InputPoint, LazyMouse, LineAnchor, PaintTarget, PaintValues, SessionScene, StrokeSetup, TipAlpha,
};
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
    /// The 3D view's camera; `None` for a stroke in the 2D view (points in texels).
    pub camera: Option<Camera>,
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
    /// The alpha image of an alpha-image tip (round and procedural tips need none).
    pub tip: Option<TipAlpha>,
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
    camera: Option<Camera>,
    surface: Arc<Surface>,
    /// Pen-down time, so samples are recorded in seconds since the stroke started.
    t0: f64,
    pub label: &'static str,
    lazy: Option<LazyMouse>,
    /// The last point painted (view coordinates).
    last: Option<[f32; 2]>,
    placement: Option<ScreenImageTransform>,
    /// What changed since the last [`ActiveStroke::take_changes`].
    changes: Changes,
}

/// Where a stroke changed the composite of its texture set.
#[derive(Debug, Clone, PartialEq)]
pub enum Changes {
    /// Anywhere (preparing the target changed more than pixels).
    Whole,
    /// In these tiles of the set's grid.
    Tiles(Vec<(u32, u32)>),
}

/// How a stroke ended.
pub struct StrokeEnd {
    /// A problem to report.
    pub problem: Option<String>,
    /// Where it ended, for the next straight line.
    pub anchor: Option<LineAnchor>,
}

/// The 3D view's projector for `camera` (`None` for a 2D-view stroke); an error if the 3D view is not ready.
fn projector_for<'a>(picking: &'a ViewPicking, mesh: &'a Mesh, camera: Option<Camera>) -> Result<Option<CpuViewProjector<'a>>, BrushError> {
    match camera {
        Some(c) => picking.projector_on(mesh, c).map(Some).ok_or(BrushError::NoView),
        None => Ok(None),
    }
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
            view: if start.camera.is_some() { ViewKind::Mesh } else { ViewKind::Uv },
            values: start.values.clone(),
            model,
            occlusion: start.occlusion,
            retouch: start.retouch,
        };
        let mut prepared = DocumentStroke::prepare(doc.doc_for_strokes(), start.target, &start.values)?;
        let whole: bool;
        let result = {
            let (mesh, set) = split(doc.doc_for_strokes(), start.set)?;
            if let Some(base) = start.base {
                prepared.see_through(set, base);
            }
            whole = prepared.changed_document();
            let projector = projector_for(picking, mesh, start.camera)?;
            let view = projector.as_ref().map(|p| p as &dyn ViewProjector);
            let projected = view.and_then(|v| projected(through, start.placement, &start.surface, v));
            let scene = SessionScene {
                surface: &start.surface.map,
                index: &start.surface.index,
                projector: view,
                factors: through.factors,
                projected: projected.as_ref(),
            };
            let extras = EngineExtras { tip: start.tip, ..EngineExtras::default() };
            prepared.start(setup, &scene, mesh, &|part| picking.is_shown(part), set).map(|s| s.with_extras(extras))
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
            changes: if whole { Changes::Whole } else { Changes::Tiles(Vec::new()) },
        };
        active.add(doc, picking, &[at], through)?;
        Ok(active)
    }

    /// What the stroke changed since the last call, for recompositing (`None`: nothing).
    pub fn take_changes(&mut self) -> Option<Changes> {
        let empty = Changes::Tiles(Vec::new());
        match std::mem::replace(&mut self.changes, empty) {
            Changes::Tiles(t) if t.is_empty() => None,
            c => Some(c),
        }
    }

    /// True for a stroke from the 2D view.
    pub fn in_uv(&self) -> bool {
        self.camera.is_none()
    }

    /// True for a projection stroke (it paints through the projection image).
    pub fn projects(&self) -> bool {
        self.placement.is_some()
    }

    /// Dabs placed so far (the performance benchmarks count them).
    #[cfg(test)]
    pub fn dabs(&self) -> usize {
        self.stroke.dabs()
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
        let projector = projector_for(picking, mesh, self.camera)?;
        let view = projector.as_ref().map(|p| p as &dyn ViewProjector);
        let projected = view.and_then(|v| projected(through, self.placement, &self.surface, v));
        let scene =
            SessionScene { surface: &self.surface.map, index: &self.surface.index, projector: view, factors: through.factors, projected: projected.as_ref() };
        let changed = self.stroke.add(&scene, &points, set)?;
        if let Changes::Tiles(t) = &mut self.changes {
            t.extend(changed.iter().copied());
            t.sort_unstable();
            t.dedup();
        }
        Ok(!changed.is_empty())
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
            Ok((mesh, set_ref)) => match projector_for(picking, mesh, camera) {
                Ok(projector) => {
                    let view = projector.as_ref().map(|p| p as &dyn ViewProjector);
                    anchor = last.map(|end| LineAnchor::at(end, view));
                    let projected = view.and_then(|v| projected(through, placement, &surface, v));
                    let scene =
                        SessionScene { surface: &surface.map, index: &surface.index, projector: view, factors: through.factors, projected: projected.as_ref() };
                    let (ended, result) = stroke.finish(&scene, set_ref);
                    (ended, result.err().map(|e| e.to_string()))
                }
                // the mesh changed under the stroke: keep what was painted, without its record
                Err(_) => (stroke.abandon(), Some("the 3D view changed during the stroke".to_string())),
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
