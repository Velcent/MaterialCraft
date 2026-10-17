//! Replaying a recorded stroke (PT-57): the stroke's samples run through a [`StrokeSession`] again with the brush,
//! values, camera and viewport recorded with it. Strokes painted in MaterialCraft carry all of that
//! ([`RecordedBrush`]); imported strokes need the importer's mapping first (roadmap step 32).

use crate::BrushError;
use crate::engine::InputPoint;
use crate::session::{Finished, SessionScene, StrokeSession, StrokeSetup};
use crate::symmetry::from_record;
use crate::target::PaintValues;
use materialcraft::mesh::{Mesh, Sphere, SurfaceMap};
use materialcraft::paint::brush::RecordedBrush;
use materialcraft::paint::stroke::{Stroke, ViewKind};
use materialcraft::projection::ViewScene;
use materialcraft::stamp::{ChannelTarget, TexelFactor, TexelIndex};

/// What a replay paints through.
pub struct ReplayScene<'a> {
    pub mesh: &'a Mesh,
    /// The picking scene of `mesh` (3D-view strokes).
    pub view: &'a ViewScene,
    pub surface: &'a SurfaceMap,
    pub index: &'a TexelIndex,
    pub factors: &'a [&'a dyn TexelFactor],
}

/// The brush a recorded stroke was painted with, if it was painted in MaterialCraft.
pub fn recorded_brush(stroke: &Stroke) -> Result<RecordedBrush, BrushError> {
    RecordedBrush::from_reference(&stroke.preset).ok_or(BrushError::NotReplayable("the stroke's brush is not one MaterialCraft recorded"))
}

/// The values a recorded brush paints.
pub fn recorded_values(brush: &RecordedBrush) -> PaintValues {
    PaintValues { channels: brush.values.iter().map(|(c, v)| (*c, *v, true)).collect(), mask: brush.mask }
}

/// Paints `stroke` again into `targets` (built with [`recorded_values`] and the stroke's tool).
pub fn replay(stroke: &Stroke, scene: &ReplayScene, targets: &mut [ChannelTarget]) -> Result<Finished, BrushError> {
    let brush = recorded_brush(stroke)?;
    let model = scene.mesh.bounding_sphere().unwrap_or(Sphere { centre: [0.0; 3], radius: 1.0 });
    let setup = StrokeSetup {
        preset: brush.preset.clone(),
        tool: stroke.tool,
        seed: stroke.seed,
        symmetry: from_record(&stroke.symmetry),
        view: stroke.view,
        values: recorded_values(&brush),
        model,
        occlusion: true,
    };
    let (w, h) = (scene.surface.width() as f32, scene.surface.height() as f32);
    let camera = stroke.camera.on_viewport(brush.viewport);
    let projector = match stroke.view {
        ViewKind::Mesh => Some(scene.view.projector(scene.mesh, camera).ok_or(BrushError::NotReplayable("the stroke was painted on another mesh"))?),
        ViewKind::Uv => None,
    };
    let session_scene = SessionScene { surface: scene.surface, index: scene.index, projector: projector.as_ref().map(|p| p as _), factors: scene.factors };
    let shown = |part: usize| scene.view.is_shown(part);
    let mut session = StrokeSession::begin(setup, &session_scene, scene.mesh, &shown, targets)?;
    let points: Vec<InputPoint> = stroke
        .samples
        .iter()
        .map(|s| {
            let pos = match stroke.view {
                ViewKind::Mesh => camera.ndc_to_screen(s.position),
                ViewKind::Uv => [s.position[0] * w, s.position[1] * h],
            };
            InputPoint { pos, pressure: s.pressure, tilt: s.tilt, time: f64::from(s.time), scale: 1.0 }
        })
        .collect();
    session.add(&session_scene, &points, targets)?;
    session.finish(&session_scene, targets)
}
