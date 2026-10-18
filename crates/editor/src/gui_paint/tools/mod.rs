//! The brush tools at work (PT-01, PT-20, PT-54, PT-55): [`Painting`] turns the viewport's stroke gesture into
//! strokes of the paint or eraser tool on the selected paintable target, one history step each. The tool choice
//! lives in the shell ([`super::shell::tools`]); the settings in [`BrushTools`]; the work in `materialcraft-brush`.
//!
//! State machine: idle → (pen down on a paintable target with a brush tool and a ready surface) painting → (pen
//! up, or any document change) idle. While painting, the set's maps are recomposited every few frames; at pen up
//! once more. A straight-line click (PT-31) is a whole stroke at once, from where the previous stroke ended
//! ([`LineAnchor`], re-projected into the current view) to the click; without a previous stroke it paints one
//! stamp. Strokes go through the lazy mouse and symmetry of [`helpers::StrokeHelpers`]; [`overlay`] draws their
//! visual aids.
//!
//! Quick mask (PT-42): while it is edited, paint and eraser strokes go into it ([`mask_stroke`]); while it is
//! applied, every document stroke is multiplied by it. The polygon fill tool ([`polygon_fill`]) turns the same
//! gesture into picks.

mod active;
pub mod brush_state;
mod events;
pub mod helpers;
mod mask_stroke;
mod overlay;
pub mod polygon_fill;
mod surfaces;

use super::document_state::OpenDocument;
use super::input::{PointerSample, StrokeInput};
use super::shell::tools::Tool;
use super::view_pick::ViewPicking;
use active::{ActiveStroke, StrokeStart};
pub use brush_state::BrushTools;
pub use events::ToolEvent;
use mask_stroke::{ActiveMaskStroke, MaskStrokeStart};
use materialcraft::camera::Camera;
use materialcraft::paint::stroke::StrokeTool;
use materialcraft::projection::ViewProjector;
use materialcraft::stamp::TexelFactor;
use materialcraft_brush::stroke::line::SNAP_DEGREES;
use materialcraft_brush::stroke::{line_points, snap_angle};
use materialcraft_brush::symmetry::to_record;
use materialcraft_brush::{LineAnchor, PaintTarget, QuickMask};
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
    /// The quick mask changed (its tint is out of date).
    pub quick_mask: bool,
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
    /// Where the last stroke ended (the next straight line's start).
    anchor: Option<LineAnchor>,
    guides: overlay::GuideCache,
    /// The session's quick mask (PT-42), honoured by every document stroke while applied.
    pub quick_mask: QuickMask,
    /// A stroke into the quick mask while it is edited.
    mask_active: Option<ActiveMaskStroke>,
    /// The polygon fill pick gesture in progress.
    gesture: Option<polygon_fill::PickGesture>,
    topologies: polygon_fill::TopologyCache,
}

impl Painting {
    /// Handles this frame's stroke input.
    pub fn frame(&mut self, input: Vec<StrokeInput>, mut f: PaintFrame) -> PaintReport {
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
        if f.tool == Tool::PolygonFill {
            let mut polygon = self.polygon_frame(input, f);
            polygon.message = polygon.message.or(report.message);
            return polygon;
        }
        if let (true, Some(tool)) = (self.quick_mask.is_editing(), tool) {
            return self.mask_frame(input, f, tool, report);
        }
        // keep the selected set's surface warm, so the first stroke does not wait
        if let (Some(_), Some((_, set))) = (tool, &f.target) {
            let _ = self.surfaces.get(f.doc, *set);
        }
        let mut painted = None;
        for ev in input {
            match ev {
                StrokeInput::Down(at) => {
                    self.end(f.doc, f.picking, &mut report);
                    if let Some(set) = self.begin(&mut f, tool, at, true, &mut report) {
                        painted = Some(set);
                    }
                }
                StrokeInput::Line { to, snap } => {
                    self.end(f.doc, f.picking, &mut report);
                    let start = self.line_start(&f);
                    let begin_at = PointerSample { pos: start.unwrap_or(to.pos), ..to };
                    if let Some(set) = self.begin(&mut f, tool, begin_at, false, &mut report) {
                        painted = Some(set);
                    }
                    if let (Some(from), Some(a)) = (start, &mut self.active) {
                        let end = if snap { snap_angle(from, to.pos, SNAP_DEGREES) } else { to.pos };
                        let samples: Vec<PointerSample> = line_points(from, end, to.pressure)
                            .iter()
                            .skip(1)
                            .map(|p| PointerSample { pos: p.pos, pressure: p.pressure, time: to.time + p.time, ..to })
                            .collect();
                        let factor = factor_for(&self.quick_mask, f.doc, a.set());
                        if let Err(e) = a.add(f.doc, f.picking, &samples, &factors(&factor)) {
                            report.message = Some(format!("The line stopped: {e}."));
                        }
                    }
                    self.end(f.doc, f.picking, &mut report);
                }
                StrokeInput::Move(at) => {
                    let Some(a) = &mut self.active else { continue };
                    let factor = factor_for(&self.quick_mask, f.doc, a.set());
                    match a.add(f.doc, f.picking, &[at], &factors(&factor)) {
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

    /// Starts a stroke at `at` (with the lazy mouse if `lazy` and it is on); returns the set it painted or tried.
    fn begin(&mut self, f: &mut PaintFrame, tool: Option<StrokeTool>, at: PointerSample, lazy: bool, report: &mut PaintReport) -> Option<usize> {
        let (Some(tool), Some((target, set)), Some(camera)) = (tool, f.target.clone(), f.camera) else { return None };
        let picking = f.picking;
        let doc = &mut *f.doc;
        let Some(surface) = self.surfaces.get(doc, set) else {
            report.message = Some("The brush is getting this texture set ready; paint again in a moment.".into());
            return None;
        };
        let set_ref = doc.doc().texture_sets.get(set)?;
        let preset = self.brush.preset(f.tool).cloned().unwrap_or_default();
        let values = self.brush.values_for(set_ref);
        self.strokes = self.strokes.wrapping_add(1);
        let helpers = &self.brush.helpers;
        let start = StrokeStart {
            target,
            set,
            preset,
            tool,
            values,
            occlusion: self.brush.occlusion,
            seed: self.strokes,
            camera,
            surface,
            symmetry: to_record(helpers.symmetry.spec()),
            lazy: if lazy { helpers.lazy.start() } else { None },
        };
        let factor = factor_for(&self.quick_mask, doc, set);
        match ActiveStroke::begin(doc, picking, start, at, &factors(&factor)) {
            Ok(a) => {
                self.active = Some(a);
            }
            Err(e) => report.message = Some(format!("Nothing was painted: {e}.")),
        }
        Some(set)
    }

    /// Where a straight line starts in the current view: the previous stroke's end, re-projected (PT-33).
    fn line_start(&self, f: &PaintFrame) -> Option<[f32; 2]> {
        let anchor = self.anchor?;
        let camera = f.camera?;
        let projector = f.picking.projector_on(&f.doc.doc().mesh, camera);
        anchor.start(projector.as_ref().map(|p| p as &dyn ViewProjector))
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

    /// True while a stroke or a pick gesture is in progress.
    pub fn is_stroking(&self) -> bool {
        self.active.is_some() || self.mask_active.is_some() || self.gesture.is_some()
    }

    /// Forgets the document's state (another document was opened or it was closed).
    pub fn forget_document(&mut self) {
        self.active = None;
        self.anchor = None;
        self.surfaces.clear();
        self.mask_active = None;
        self.gesture = None;
        self.quick_mask.forget();
        self.topologies = Default::default();
    }

    fn end(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, report: &mut PaintReport) {
        self.gesture = None;
        self.end_mask(doc, picking, report);
        let Some(a) = self.active.take() else { return };
        let set = a.set();
        let factor = factor_for(&self.quick_mask, doc, set);
        let end = a.end(doc, picking, &factors(&factor));
        if let Some(problem) = end.problem {
            report.message = Some(format!("The stroke had a problem: {problem}."));
        }
        if end.anchor.is_some() {
            self.anchor = end.anchor;
        }
        report.refresh = Some(set);
        self.last_refresh = Some(Instant::now());
    }
}

/// The quick mask's factor for strokes into texture set `set`, while it is applied.
fn factor_for<'a>(mask: &'a QuickMask, doc: &OpenDocument, set: usize) -> Option<materialcraft::stamp::QuickMask<'a>> {
    let s = doc.doc().texture_sets.get(set)?;
    mask.factor(&s.id, s.resolution)
}

fn factors<'a>(factor: &'a Option<materialcraft::stamp::QuickMask<'a>>) -> Vec<&'a dyn TexelFactor> {
    factor.iter().map(|f| f as &dyn TexelFactor).collect()
}

impl Painting {
    /// This frame's brush input while the quick mask is edited: strokes paint the mask (PT-42).
    fn mask_frame(&mut self, input: Vec<StrokeInput>, f: PaintFrame, tool: StrokeTool, mut report: PaintReport) -> PaintReport {
        let Some((_, set)) = f.target.clone() else { return report };
        let mut painted = false;
        for ev in input {
            match ev {
                StrokeInput::Down(at) | StrokeInput::Line { to: at, .. } => {
                    self.end_mask(f.doc, f.picking, &mut report);
                    let Some(camera) = f.camera else { continue };
                    let Some(surface) = self.surfaces.get(f.doc, set) else {
                        report.message = Some("The brush is getting this texture set ready; paint again in a moment.".into());
                        continue;
                    };
                    self.strokes = self.strokes.wrapping_add(1);
                    let start = MaskStrokeStart {
                        set,
                        preset: self.brush.preset(f.tool).cloned().unwrap_or_default(),
                        tool,
                        occlusion: self.brush.occlusion,
                        seed: self.strokes,
                        camera,
                        surface,
                        symmetry: to_record(self.brush.helpers.symmetry.spec()),
                    };
                    match ActiveMaskStroke::begin(&mut self.quick_mask, f.doc, f.picking, start, at) {
                        Ok(a) => {
                            self.mask_active = Some(a);
                            painted = true;
                        }
                        Err(e) => report.message = Some(format!("Nothing was painted into the quick mask: {e}.")),
                    }
                }
                StrokeInput::Move(at) => {
                    let Some(a) = &mut self.mask_active else { continue };
                    match a.add(&mut self.quick_mask, f.doc, f.picking, &[at]) {
                        Ok(changed) => painted |= changed,
                        Err(e) => {
                            report.message = Some(format!("The stroke stopped: {e}."));
                            self.end_mask(f.doc, f.picking, &mut report);
                        }
                    }
                }
                StrokeInput::Up => self.end_mask(f.doc, f.picking, &mut report),
            }
        }
        if painted && self.last_refresh.is_none_or(|t| t.elapsed() >= REFRESH) {
            report.refresh = Some(set);
            report.quick_mask = true;
            self.last_refresh = Some(Instant::now());
        }
        report.busy = self.mask_active.is_some() || self.surfaces.is_building();
        report
    }

    fn end_mask(&mut self, doc: &OpenDocument, picking: &ViewPicking, report: &mut PaintReport) {
        let Some(a) = self.mask_active.take() else { return };
        let set = a.set();
        if let Some(problem) = a.end(&mut self.quick_mask, doc, picking) {
            report.message = Some(format!("The quick-mask stroke had a problem: {problem}."));
        }
        report.refresh = Some(set);
        report.quick_mask = true;
        self.last_refresh = Some(Instant::now());
    }
}
