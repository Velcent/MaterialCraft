//! Replaying a recorded stroke (PT-57): the stroke's samples run through a [`StrokeSession`] again with the brush,
//! values, camera and viewport recorded with it. Strokes painted in MaterialCraft carry all of that
//! ([`RecordedBrush`], [`replay`]); imported strokes come with a brush the importer mapped ([`ReplayBrush`],
//! [`replay_imported`], roadmap step 32), painted the same way without the editor.

use crate::BrushError;
use crate::engine::{EngineExtras, InputPoint};
use crate::session::{Finished, SessionScene, StrokeSession, StrokeSetup};
use crate::stroke::LazyMouse;
use crate::target::PaintValues;
use crate::tips::{AlphaSource, NoAlphas, alpha_for};
use materialcraft::mesh::{Mesh, Sphere, SurfaceMap};
use materialcraft::paint::brush::{BrushPreset, RecordedBrush, Retouch};
use materialcraft::paint::stroke::{Stroke, StrokeTool, ViewKind};
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

/// The brush of an imported stroke, as the importer mapped it: our preset, what the preset can't say, the lazy
/// mouse and the view the stroke's samples are laid out on.
#[derive(Debug, Clone, PartialEq)]
pub struct ReplayBrush {
    pub preset: BrushPreset,
    pub extras: EngineExtras,
    /// The lazy mouse's string as a fraction of the frame's height (the viewport for 3D-view strokes, the texture
    /// for 2D-view strokes); 0 = off. The samples are the pointer's path, the brush trails it.
    pub lazy: f32,
    /// Height of the viewport 3D-view samples are replayed on (screen units; the width follows the stored
    /// projection's aspect). Screen-sized brushes and the lazy mouse scale with it.
    pub viewport_height: f32,
    /// Test occlusion (3D view).
    pub occlusion: bool,
    /// The sphere model-sized brushes are a fraction of (default: the bounding sphere of the scene's mesh).
    pub model: Option<Sphere>,
}

/// The brush a recorded stroke was painted with, if it was painted in MaterialCraft.
pub fn recorded_brush(stroke: &Stroke) -> Result<RecordedBrush, BrushError> {
    RecordedBrush::from_reference(&stroke.preset).ok_or(BrushError::NotReplayable("the stroke's brush is not one MaterialCraft recorded"))
}

/// The values a recorded brush paints.
pub fn recorded_values(brush: &RecordedBrush) -> PaintValues {
    PaintValues { channels: brush.values.iter().map(|(c, v)| (*c, *v, true)).collect(), mask: brush.mask }
}

/// Paints `stroke` again into `targets` (built with [`recorded_values`] and [`crate::target::mode_for`] of the stroke's
/// tool and recorded retouch settings).
/// Alpha-image tips paint round here; [`replay_with`] finds them.
pub fn replay(stroke: &Stroke, scene: &ReplayScene, targets: &mut [ChannelTarget]) -> Result<Finished, BrushError> {
    replay_with(stroke, scene, targets, &NoAlphas)
}

/// [`replay`], with the alpha image of a brush with an alpha-image tip taken from `alphas`.
pub fn replay_with(stroke: &Stroke, scene: &ReplayScene, targets: &mut [ChannelTarget], alphas: &dyn AlphaSource) -> Result<Finished, BrushError> {
    if stroke.tool == StrokeTool::Projection {
        // the record keeps the camera but not the projected image and its placement (PJ-28)
        return Err(BrushError::NotReplayable("projection strokes do not record their image yet"));
    }
    let brush = recorded_brush(stroke)?;
    let camera = stroke.camera.on_viewport(brush.viewport);
    let extras = EngineExtras { tip: alpha_for(&brush.preset.tip, alphas), ..EngineExtras::default() };
    let painting =
        Painting { preset: &brush.preset, extras: &extras, lazy: None, values: recorded_values(&brush), occlusion: true, model: None, retouch: brush.retouch };
    paint(stroke, &painting, camera, scene, targets)
}

/// Paints an imported `stroke` with `brush` and `values` into `targets` (built from `values` and the stroke's
/// tool). The camera of a 3D-view stroke goes on a viewport [`ReplayBrush::viewport_height`] high.
pub fn replay_imported(
    stroke: &Stroke,
    brush: &ReplayBrush,
    values: &PaintValues,
    scene: &ReplayScene,
    targets: &mut [ChannelTarget],
) -> Result<Finished, BrushError> {
    let height = if brush.viewport_height.is_finite() && brush.viewport_height >= 1.0 { brush.viewport_height } else { 1024.0 };
    let camera = stroke.camera.on_viewport_of_height(height);
    let frame_height = match stroke.view {
        ViewKind::Mesh => height,
        ViewKind::Uv => scene.surface.height() as f32,
    };
    let lazy = (brush.lazy.is_finite() && brush.lazy > 0.0).then_some(brush.lazy * frame_height);
    let painting = Painting {
        preset: &brush.preset,
        extras: &brush.extras,
        lazy,
        values: values.clone(),
        occlusion: brush.occlusion,
        model: brush.model,
        retouch: Retouch::default(),
    };
    paint(stroke, &painting, camera, scene, targets)
}

/// How a stroke is painted, from either kind of brush.
struct Painting<'a> {
    preset: &'a BrushPreset,
    extras: &'a EngineExtras,
    /// Lazy mouse string in frame units.
    lazy: Option<f32>,
    values: PaintValues,
    occlusion: bool,
    model: Option<Sphere>,
    /// Smudge and blur settings (recorded strokes only).
    retouch: Retouch,
}

fn paint(
    stroke: &Stroke,
    painting: &Painting,
    camera: materialcraft::camera::Camera,
    scene: &ReplayScene,
    targets: &mut [ChannelTarget],
) -> Result<Finished, BrushError> {
    let model = painting.model.or_else(|| scene.mesh.bounding_sphere()).unwrap_or(Sphere { centre: [0.0; 3], radius: 1.0 });
    let setup = StrokeSetup {
        preset: painting.preset.clone(),
        tool: stroke.tool,
        seed: stroke.seed,
        symmetry: stroke.symmetry,
        view: stroke.view,
        values: painting.values.clone(),
        model,
        occlusion: painting.occlusion,
        retouch: painting.retouch,
    };
    let (w, h) = (scene.surface.width() as f32, scene.surface.height() as f32);
    let projector = match stroke.view {
        ViewKind::Mesh => Some(scene.view.projector(scene.mesh, camera).ok_or(BrushError::NotReplayable("the stroke was painted on another mesh"))?),
        ViewKind::Uv => None,
    };
    let session_scene =
        SessionScene { surface: scene.surface, index: scene.index, projector: projector.as_ref().map(|p| p as _), factors: scene.factors, projected: None };
    let shown = |part: usize| scene.view.is_shown(part);
    let mut session = StrokeSession::begin(setup, &session_scene, scene.mesh, &shown, targets)?.with_extras(painting.extras.clone());
    let points: Vec<InputPoint> = stroke
        .samples
        .iter()
        .map(|s| {
            let pos = match stroke.view {
                ViewKind::Mesh => camera.ndc_to_screen(s.position),
                ViewKind::Uv => [s.position[0] * w, s.position[1] * h],
            };
            InputPoint { pos, pressure: s.pressure, tilt: s.tilt, rotation: s.rotation, time: f64::from(s.time), scale: 1.0 }
        })
        .collect();
    let points = match painting.lazy.and_then(LazyMouse::new) {
        Some(mut lazy) => lazy.push(&points),
        None => points,
    };
    // point by point, as the pen delivered them: the dabs of one call are held with their footprints until
    // painted, and a long stroke with a big brush would hold thousands of large ones at once
    for p in points.chunks(1) {
        session.add(&session_scene, p, targets)?;
    }
    session.finish(&session_scene, targets)
}
