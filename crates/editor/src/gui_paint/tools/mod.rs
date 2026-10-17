//! The brush tools at work (PT-01, PT-20, PT-54, PT-55): [`Painting`] turns the viewport's stroke gesture into
//! strokes of the paint or eraser tool on the selected paintable target, one history step each. The tool choice
//! lives in the shell ([`super::shell::tools`]); the settings in [`BrushTools`]; the work in `materialcraft-brush`.
//!
//! State machine: idle → (pen down on a paintable target with a brush tool and a ready surface) painting → (pen
//! up, or any document change) idle. While painting, the set's maps are recomposited every few frames; at pen up
//! once more.

mod active;
pub mod brush_state;
mod surfaces;

use super::document_state::OpenDocument;
use super::input::StrokeInput;
use super::shell::tools::Tool;
use super::view_pick::ViewPicking;
use active::{ActiveStroke, StrokeStart};
pub use brush_state::BrushTools;
use materialcraft::camera::Camera;
use materialcraft::paint::stroke::StrokeTool;
use materialcraft_brush::PaintTarget;
use std::time::{Duration, Instant};
use surfaces::SurfaceCache;

/// How often the painted set is recomposited during a stroke.
const REFRESH: Duration = Duration::from_millis(120);

/// What one frame of painting asks of the screen.
#[derive(Debug, Default, PartialEq)]
pub struct PaintReport {
    /// Recomposite this texture set.
    pub refresh: Option<usize>,
    /// Something to tell the user.
    pub message: Option<String>,
    /// True while a stroke is in progress (keep frames coming).
    pub busy: bool,
}

/// What the tools work with this frame.
pub struct PaintFrame<'a> {
    pub doc: &'a mut OpenDocument,
    pub picking: &'a ViewPicking,
    /// The view's camera (screen positions relative to the view's corner).
    pub camera: Option<Camera>,
    pub tool: Tool,
    /// The selected paintable target and its texture set's index.
    pub target: Option<(PaintTarget, usize)>,
}

/// The brush tools' session state.
#[derive(Default)]
pub struct Painting {
    pub brush: BrushTools,
    surfaces: SurfaceCache,
    active: Option<ActiveStroke>,
    last_refresh: Option<Instant>,
    strokes: u32,
}

impl Painting {
    /// Handles this frame's stroke input.
    pub fn frame(&mut self, input: Vec<StrokeInput>, f: PaintFrame) -> PaintReport {
        let mut report = PaintReport::default();
        self.surfaces.poll();
        if let Some(e) = self.surfaces.error.take() {
            report.message = Some(format!("The brush can't paint this texture set: {e}."));
        }
        let tool = match f.tool {
            Tool::Paint => Some(StrokeTool::Paint),
            Tool::Eraser => Some(StrokeTool::Eraser),
            _ => None,
        };
        // keep the selected set's surface warm, so the first stroke does not wait
        if let (Some(_), Some((_, set))) = (tool, &f.target) {
            let _ = self.surfaces.get(f.doc, *set);
        }
        let mut painted = None;
        for ev in input {
            match ev {
                StrokeInput::Down(at) => {
                    self.end(f.doc, f.picking, &mut report);
                    let (Some(tool), Some((target, set)), Some(camera)) = (tool, f.target.clone(), f.camera) else { continue };
                    let Some(surface) = self.surfaces.get(f.doc, set) else {
                        report.message = Some("The brush is getting this texture set ready; paint again in a moment.".into());
                        continue;
                    };
                    let Some(set_ref) = f.doc.doc().texture_sets.get(set) else { continue };
                    let preset = self.brush.preset(f.tool).cloned().unwrap_or_default();
                    let values = self.brush.values_for(set_ref);
                    self.strokes = self.strokes.wrapping_add(1);
                    let start = StrokeStart { target, set, preset, tool, values, occlusion: self.brush.occlusion, seed: self.strokes, camera, surface };
                    match ActiveStroke::begin(f.doc, f.picking, start, at) {
                        Ok(a) => {
                            painted = Some(a.set());
                            self.active = Some(a);
                        }
                        Err(e) => {
                            report.message = Some(format!("Nothing was painted: {e}."));
                            painted = Some(set);
                        }
                    }
                }
                StrokeInput::Move(at) => {
                    let Some(a) = &mut self.active else { continue };
                    match a.add(f.doc, f.picking, &[at]) {
                        Ok(true) => painted = Some(a.set()),
                        Ok(false) => {}
                        Err(e) => {
                            report.message = Some(format!("The stroke stopped: {e}."));
                            self.end(f.doc, f.picking, &mut report);
                        }
                    }
                }
                StrokeInput::Up => self.end(f.doc, f.picking, &mut report),
            }
        }
        if report.refresh.is_none()
            && let Some(set) = painted
            && self.last_refresh.is_none_or(|t| t.elapsed() >= REFRESH)
        {
            report.refresh = Some(set);
            self.last_refresh = Some(Instant::now());
        }
        report.busy = self.active.is_some() || self.surfaces.is_building();
        report
    }

    /// Ends a stroke in progress (pen up, or before anything else changes the document).
    pub fn finish_stroke(&mut self, doc: &mut OpenDocument, picking: &ViewPicking) -> PaintReport {
        let mut report = PaintReport::default();
        self.end(doc, picking, &mut report);
        report
    }

    /// True once texture set `set`'s surface is built (starts building it otherwise).
    pub fn surface_ready(&mut self, doc: &OpenDocument, set: usize) -> bool {
        self.surfaces.get(doc, set).is_some()
    }

    /// True while a stroke is in progress.
    pub fn is_stroking(&self) -> bool {
        self.active.is_some()
    }

    /// Forgets the document's state (another document was opened or it was closed).
    pub fn forget_document(&mut self) {
        self.active = None;
        self.surfaces.clear();
    }

    fn end(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, report: &mut PaintReport) {
        let Some(a) = self.active.take() else { return };
        let set = a.set();
        if let Some(problem) = a.end(doc, picking) {
            report.message = Some(format!("The stroke had a problem: {problem}."));
        }
        report.refresh = Some(set);
        self.last_refresh = Some(Instant::now());
    }
}
