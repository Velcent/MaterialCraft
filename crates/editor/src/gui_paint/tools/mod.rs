//! The brush tools at work (PT-01, PT-20, PT-54, PT-55): [`Painting`] turns the viewport's stroke gesture into
//! strokes of the paint, eraser, smudge (or blur) and clone tools on the selected paintable target, one history step
//! each. The tool choice lives in the shell ([`super::shell::tools`]); the settings in [`BrushTools`]; the work in
//! `materialcraft-brush`.
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
//!
//! Retouch tools ([`retouch`], PT-23…PT-30): smudge and blur run local kernels, clone copies from the source marked
//! with its key (aligned or fixed). Strokes on a see-through layer start from the composite below it; the layer is
//! painted again from its record whenever the layers below it change.
//!
//! Stencil and projection (PT-15, PJ-26, [`screen_image`]): a stencil laid over the view masks every document
//! stroke; the projection tool paints the projection image's channels through the paint tool's brush. Strokes
//! through either have no symmetry (ADR 0008 mc2-q-pt-04c). The material picker ([`material_picker`]) is handled
//! by the screen, which owns the composite's fill context.

mod active;
pub mod brush_state;
mod events;
pub mod helpers;
mod mask_stroke;
pub mod material_picker;
mod overlay;
pub mod polygon_fill;
mod retouch;
pub mod screen_image;
mod surfaces;

use super::document_state::OpenDocument;
use super::input::{PointerSample, StrokeInput};
use super::shell::tools::Tool;
use super::view_pick::ViewPicking;
use active::{ActiveStroke, StrokeStart, Through};
pub use brush_state::BrushTools;
pub use events::ToolEvent;
use mask_stroke::{ActiveMaskStroke, MaskStrokeStart};
use materialcraft::camera::Camera;
use materialcraft::paint::stroke::StrokeTool;
use materialcraft::projection::ViewProjector;
use materialcraft::stamp::{Stencil, TexelFactor};
use materialcraft::symmetry::SymmetrySpec;
use materialcraft_brush::stroke::line::SNAP_DEGREES;
use materialcraft_brush::stroke::{line_points, snap_angle};
use materialcraft_brush::symmetry::to_record;
use materialcraft_brush::{LineAnchor, PaintTarget, QuickMask};
use screen_image::ScreenImages;
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
    /// The clone source and the see-through layers' cache.
    retouch: retouch::RetouchState,
    /// The stencil and the projection tool's image over the view.
    pub images: ScreenImages,
}

impl Painting {
    /// Handles this frame's stroke input.
    pub fn frame(&mut self, input: Vec<StrokeInput>, mut f: PaintFrame) -> PaintReport {
        let mut report = PaintReport::default();
        self.surfaces.poll();
        if let Some(e) = self.surfaces.error.take() {
            report.message = Some(format!("The brush can't paint this texture set: {e}."));
        }
        let tool = self.brush.stroke_tool(f.tool);
        // see-through layers follow the layers below them (PT-25)
        if !self.is_stroking()
            && let Some((_, set)) = &f.target
        {
            self.keep_see_through(f.doc, f.picking, *set, &mut report);
        }
        if let Some(camera) = f.camera {
            self.images.place(camera.viewport);
        }
        if f.tool == Tool::PolygonFill {
            let mut polygon = self.polygon_frame(input, f);
            polygon.message = polygon.message.or(report.message);
            polygon.refresh = polygon.refresh.or(report.refresh);
            return polygon;
        }
        if let (true, Some(tool)) = (self.quick_mask.is_editing(), tool) {
            if !matches!(tool, StrokeTool::Paint | StrokeTool::Eraser) {
                if input.iter().any(|ev| matches!(ev, StrokeInput::Down(_))) {
                    report.message = Some("Only the paint and eraser tools work on the quick mask while it is edited.".into());
                }
                return report;
            }
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
                        let (factor, stencil) = (factor_for(&self.quick_mask, f.doc, a.set()), self.images.stencil_factor());
                        let fs = factors(&factor, &stencil);
                        if let Err(e) = a.add(f.doc, f.picking, &samples, through(&fs, &self.images, f.tool)) {
                            report.message = Some(format!("The line stopped: {e}."));
                        }
                    }
                    self.end(f.doc, f.picking, &mut report);
                }
                StrokeInput::Move(at) => {
                    let Some(a) = &mut self.active else { continue };
                    let (factor, stencil) = (factor_for(&self.quick_mask, f.doc, a.set()), self.images.stencil_factor());
                    let fs = factors(&factor, &stencil);
                    match a.add(f.doc, f.picking, &[at], through(&fs, &self.images, f.tool)) {
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
        let retouch = match self.retouch_for(doc, picking, camera, tool, at, set) {
            Ok(r) => r,
            Err(why) => {
                report.message = Some(format!("Nothing was painted: {why}."));
                return Some(set);
            }
        };
        // a see-through layer is brought up to date first, and the stroke works on the composite below it
        let base = self.see_through_base(doc, picking, &surface, &target, set);
        let set_ref = doc.doc().texture_sets.get(set)?;
        let preset = self.brush.preset(f.tool).cloned().unwrap_or_default();
        let mut values = self.brush.values_for(set_ref);
        let placement = match (tool, &mut self.images.projection) {
            (StrokeTool::Projection, Some(image)) => {
                // only the channels the projection provides are painted
                let provided = image.channels();
                values.channels.iter_mut().for_each(|(c, _, on)| *on &= provided.contains(c));
                Some(image.placement(camera.viewport))
            }
            (StrokeTool::Projection, None) => {
                report.message = Some("Load a projection image or drop an image or material from the assets onto the view first.".into());
                return None;
            }
            _ => None,
        };
        self.strokes = self.strokes.wrapping_add(1);
        let helpers = &self.brush.helpers;
        // projection painting and stencils have no symmetry (ADR 0008 mc2-q-pt-04c)
        let symmetry = if placement.is_some() || self.images.stencil.is_some() { SymmetrySpec::None } else { helpers.symmetry.spec() };
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
            symmetry: to_record(symmetry),
            lazy: if lazy { helpers.lazy.start() } else { None },
            retouch,
            base,
            placement,
        };
        let (factor, stencil) = (factor_for(&self.quick_mask, doc, set), self.images.stencil_factor());
        let fs = factors(&factor, &stencil);
        match ActiveStroke::begin(doc, picking, start, at, through(&fs, &self.images, f.tool)) {
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

    /// Texture set `set`'s surface once built (starts building it otherwise), for other users of the painted set's
    /// surface (the projection gizmo's UV handle).
    pub fn surface_of(&mut self, doc: &OpenDocument, set: usize) -> Option<std::sync::Arc<surfaces::Surface>> {
        self.surfaces.get(doc, set)
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
        self.retouch.clear();
    }

    /// Paints the see-through layers of `set` again if the layers below them changed (once its surface is ready).
    fn keep_see_through(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, set: usize, report: &mut PaintReport) {
        let has_any = doc.doc().texture_sets.get(set).is_some_and(|s| !materialcraft::paint::see_through::see_through_layers(s).is_empty());
        if !has_any {
            return;
        }
        let Some(surface) = self.surfaces.get(doc, set) else { return };
        let (repainted, problem) = self.refresh_see_through(doc, picking, &surface, set);
        if repainted {
            report.refresh = Some(set);
        }
        if problem.is_some() {
            report.message = problem;
        }
    }

    fn end(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, report: &mut PaintReport) {
        self.gesture = None;
        self.end_mask(doc, picking, report);
        let Some(a) = self.active.take() else { return };
        let set = a.set();
        let (factor, stencil) = (factor_for(&self.quick_mask, doc, set), self.images.stencil_factor());
        let fs = factors(&factor, &stencil);
        let projection = a.projects().then_some(self.images.projection.as_ref()).flatten();
        let end = a.end(doc, picking, Through { factors: &fs, projection });
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

/// The factors of a document stroke: the quick mask while applied, the stencil while there is one.
fn factors<'a>(factor: &'a Option<materialcraft::stamp::QuickMask<'a>>, stencil: &'a Option<Stencil<'a>>) -> Vec<&'a dyn TexelFactor> {
    let mask = factor.iter().map(|f| f as &dyn TexelFactor);
    mask.chain(stencil.iter().map(|s| s as &dyn TexelFactor)).collect()
}

/// What a stroke of `tool` paints through.
fn through<'a>(factors: &'a [&'a dyn TexelFactor], images: &'a ScreenImages, tool: Tool) -> Through<'a> {
    Through { factors, projection: if tool == Tool::Projection { images.projection.as_ref() } else { None } }
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
