//! The retouch tools' session state (PT-23…PT-30): the clone source (marked with its key while hovering the
//! model, PT-27; aligned or fixed, PT-28) and see-through layers ([`SeeThroughCache`]): before a stroke on one and
//! whenever the layers below it change, it is painted again from its record on the composite below (PT-25).

use super::super::document_state::OpenDocument;
use super::super::view_pick::ViewPicking;
use super::Painting;
use super::surfaces::Surface;
use crate::gui_paint::input::PointerSample;
use materialcraft::camera::Camera;
use materialcraft::mesh::Vec3;
use materialcraft::paint::brush::Retouch;
use materialcraft::paint::stroke::StrokeTool;
use materialcraft::paint::{Document, LayerKind, NO_FILL_CONTEXT};
use materialcraft::projection::ViewProjector;
use materialcraft_brush::{Base, BelowSource, CloneSource, PaintTarget, ReplayScene, SeeThroughCache, TargetPart, texel_under};

/// The clone source and the see-through layers' cache.
#[derive(Default)]
pub struct RetouchState {
    pub clone: CloneSource,
    /// Where the source was marked on the model (object space), for the overlay.
    pub marker: Option<Vec3>,
    pub see_through: SeeThroughCache,
}

impl RetouchState {
    /// Forgets the document's state.
    pub fn clear(&mut self) {
        self.clone.clear();
        self.marker = None;
        self.see_through.clear();
    }
}

/// True if `target` is the content of a see-through paint layer.
pub fn is_see_through(doc: &Document, target: &PaintTarget) -> bool {
    let layer = doc.texture_set(&target.layer.set).and_then(|s| s.layer(&target.layer.layer));
    target.part == TargetPart::Content && matches!(layer.map(|l| &l.kind), Some(LayerKind::Paint(p)) if p.see_through)
}

impl Painting {
    /// Marks the surface under `pointer` (view coordinates) in texture set `set` as the clone source. Returns what
    /// to tell the user.
    pub fn mark_clone_source(&mut self, doc: &OpenDocument, picking: &ViewPicking, camera: Option<Camera>, pointer: Option<[f32; 2]>, set: usize) -> String {
        let d = doc.doc();
        let (Some(camera), Some(pointer), Some(s)) = (camera, pointer, d.texture_sets.get(set)) else {
            return "Hover the model and press the clone source key to mark where clone strokes copy from.".into();
        };
        let Some(projector) = picking.projector_on(&d.mesh, camera) else { return "The 3D view is not ready yet; try again in a moment.".into() };
        let Some(texel) = texel_under(&projector, pointer, s.resolution) else {
            return "There is no paint of this texture set under the pointer to clone from.".into();
        };
        self.retouch.clone.mode = self.brush.retouch.clone_mode;
        self.retouch.clone.mark(&s.id, texel);
        self.retouch.marker = projector.pick(pointer).map(|h| h.position);
        "Clone source marked.".into()
    }

    /// The retouch settings of a stroke of `tool` starting at `at` in texture set `set`: for clone, the offset from
    /// the source. `Err` says why the stroke can't start.
    pub(super) fn retouch_for(
        &mut self,
        doc: &OpenDocument,
        picking: &ViewPicking,
        camera: Option<Camera>,
        tool: StrokeTool,
        at: PointerSample,
        set: usize,
    ) -> Result<Retouch, String> {
        let retouch = Retouch { strength: self.brush.retouch.strength.clamp(0.0, 1.0), ..Retouch::default() };
        if tool != StrokeTool::Clone {
            return Ok(retouch);
        }
        let d = doc.doc();
        let s = d.texture_sets.get(set).ok_or("the texture set is gone")?;
        // a 2D-view stroke's points are texels already
        let start = match camera {
            Some(camera) => {
                let projector = picking.projector_on(&d.mesh, camera).ok_or("the 3D view is not ready")?;
                texel_under(&projector, at.pos, s.resolution).ok_or("start the clone stroke on the model")?
            }
            None => at.pos,
        };
        self.retouch.clone.mode = self.brush.retouch.clone_mode;
        let offset =
            self.retouch.clone.offset_for_stroke(&s.id, start).ok_or("mark a clone source first: hover the model and press the clone source key (V)")?;
        Ok(Retouch { clone_offset: offset, ..retouch })
    }

    /// Paints the see-through layers of texture set `set` again where the layers below them changed. Returns a
    /// problem to report, and whether anything was painted.
    pub(super) fn refresh_see_through(&mut self, doc: &mut OpenDocument, picking: &ViewPicking, surface: &Surface, set: usize) -> (bool, Option<String>) {
        let Some(view) = picking.scene() else { return (false, None) };
        let Document { mesh, texture_sets, .. } = doc.doc_for_see_through();
        let Some(s) = texture_sets.get_mut(set) else { return (false, None) };
        if materialcraft::paint::see_through::see_through_layers(s).is_empty() {
            return (false, None);
        }
        let below = BelowSource { effects: materialcraft::generators::builtin(), fills: &NO_FILL_CONTEXT };
        let scene = ReplayScene { mesh, view, surface: &surface.map, index: &surface.index, factors: &[] };
        let done = self.retouch.see_through.refresh(s, below, &scene);
        let problem = done.problems.first().map(|(layer, e)| format!("The see-through layer \"{layer}\" could not be painted again: {e}."));
        (!done.repainted.is_empty(), problem)
    }

    /// The composite below see-through `target`, after bringing the layer up to date; `None` for other targets.
    pub(super) fn see_through_base(
        &mut self,
        doc: &mut OpenDocument,
        picking: &ViewPicking,
        surface: &Surface,
        target: &PaintTarget,
        set: usize,
    ) -> Option<Base> {
        if !is_see_through(doc.doc(), target) {
            return None;
        }
        self.refresh_see_through(doc, picking, surface, set);
        let s = doc.doc().texture_sets.get(set)?;
        self.retouch.see_through.base(s, &target.layer.layer).cloned()
    }

    /// Where the clone source shows in the view through `camera` while the clone tool is chosen.
    pub(super) fn clone_marker(&self, doc: &OpenDocument, picking: &ViewPicking, camera: Option<Camera>) -> Option<[f32; 2]> {
        let projector = picking.projector_on(&doc.doc().mesh, camera?)?;
        projector.to_screen(self.retouch.marker?).map(|s| s.pos)
    }
}
