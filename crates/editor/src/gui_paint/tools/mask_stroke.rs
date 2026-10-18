//! [`ActiveMaskStroke`]: a paint or eraser stroke into the quick mask while it is being edited (PT-42): the brush
//! crate's [`QuickMaskStroke`] plus what it paints through (the set's surface, the camera at pen down).

use super::super::document_state::OpenDocument;
use super::super::view_pick::ViewPicking;
use super::surfaces::Surface;
use crate::gui_paint::input::PointerSample;
use materialcraft::camera::Camera;
use materialcraft::mesh::Sphere;
use materialcraft::paint::brush::BrushPreset;
use materialcraft::paint::stroke::{StrokeTool, Symmetry, ViewKind};
use materialcraft::projection::ViewProjector;
use materialcraft_brush::{BrushError, InputPoint, PaintValues, QuickMask, QuickMaskStroke, SessionScene, StrokeSetup};
use std::sync::Arc;

/// How to start a quick-mask stroke.
pub struct MaskStrokeStart {
    pub set: usize,
    pub preset: BrushPreset,
    pub tool: StrokeTool,
    pub occlusion: bool,
    pub seed: u32,
    pub camera: Camera,
    pub surface: Arc<Surface>,
    pub symmetry: Symmetry,
}

pub struct ActiveMaskStroke {
    stroke: QuickMaskStroke,
    set: usize,
    camera: Camera,
    surface: Arc<Surface>,
    t0: f64,
}

impl ActiveMaskStroke {
    /// Pen down at `at`: starts the stroke into the set's quick mask and paints its first sample.
    pub fn begin(
        mask: &mut QuickMask,
        doc: &OpenDocument,
        picking: &ViewPicking,
        start: MaskStrokeStart,
        at: PointerSample,
    ) -> Result<ActiveMaskStroke, BrushError> {
        let d = doc.doc();
        let set = d.texture_sets.get(start.set).ok_or(BrushError::NotPaintable("the texture set is gone"))?;
        let setup = StrokeSetup {
            preset: start.preset,
            tool: start.tool,
            seed: start.seed,
            symmetry: start.symmetry,
            view: ViewKind::Mesh,
            values: PaintValues { channels: Vec::new(), mask: 1.0 },
            model: d.mesh.bounding_sphere().unwrap_or(Sphere { centre: [0.0; 3], radius: 1.0 }),
            occlusion: start.occlusion,
        };
        let projector = picking.projector_on(&d.mesh, start.camera).ok_or(BrushError::NoView)?;
        let scene = SessionScene { surface: &start.surface.map, index: &start.surface.index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
        let stroke = QuickMaskStroke::begin(mask, &set.id, set.resolution, setup, &scene, &d.mesh, &|part| picking.is_shown(part))?;
        let mut active = ActiveMaskStroke { stroke, set: start.set, camera: start.camera, surface: start.surface, t0: at.time };
        active.add(mask, doc, picking, &[at])?;
        Ok(active)
    }

    /// The texture set whose quick mask is painted.
    pub fn set(&self) -> usize {
        self.set
    }

    /// Paints more samples; returns whether any pixel changed.
    pub fn add(&mut self, mask: &mut QuickMask, doc: &OpenDocument, picking: &ViewPicking, samples: &[PointerSample]) -> Result<bool, BrushError> {
        let points: Vec<InputPoint> = samples
            .iter()
            .map(|s| InputPoint { pressure: s.pressure, tilt: s.tilt, rotation: s.rotation, ..InputPoint::at(s.pos, (s.time - self.t0).max(0.0)) })
            .collect();
        if points.is_empty() {
            return Ok(false);
        }
        let mesh = &doc.doc().mesh;
        let projector = picking.projector_on(mesh, self.camera).ok_or(BrushError::NoView)?;
        let scene = SessionScene { surface: &self.surface.map, index: &self.surface.index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
        Ok(!self.stroke.add(mask, &scene, &points)?.is_empty())
    }

    /// Pen up: paints the last stamps; returns a problem to report.
    pub fn end(self, mask: &mut QuickMask, doc: &OpenDocument, picking: &ViewPicking) -> Option<String> {
        let mesh = &doc.doc().mesh;
        let projector = picking.projector_on(mesh, self.camera)?;
        let scene = SessionScene { surface: &self.surface.map, index: &self.surface.index, projector: Some(&projector as &dyn ViewProjector), factors: &[] };
        self.stroke.finish(mask, &scene).err().map(|e| e.to_string())
    }
}
