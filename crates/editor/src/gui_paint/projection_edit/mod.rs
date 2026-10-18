//! Editing the selected fill's projection in the view (roadmap step 38, PJ-23 … PJ-25): the 3D gizmo of the
//! volume modes (move, rotate, scale; stepped with Ctrl; local or world axes), snapping planar and warp volumes
//! onto the surface (Shift while dragging the centre), the UV handle of UV mode laid over the mesh, and the warp
//! grid's points (step 43, PJ-20). The maths is `materialcraft::projection::gizmo`; this module turns the viewport's
//! pointer into drags and every drag into edits recorded under one history key, so one drag is one undo step.
//!
//! - [`GizmoSettings`]: what the gizmo shows and its steps (session settings, edited in the projection panel);
//! - [`view`]: the gizmo on screen this frame and what the pointer is over ([`warp_view`]: the warp grid);
//!   [`draw`]: drawing it.

pub mod draw;
#[cfg(test)]
mod tests;
pub mod view;
pub mod warp_view;

use super::document_state::OpenDocument;
use super::shared_mesh::SharedMesh;
use materialcraft::history::CoalesceKey;
use materialcraft::mesh::SurfaceMap;
use materialcraft::paint::edit::EditError;
use materialcraft::paint::edit::ops::{FillRef, set_projection};
use materialcraft::projection::gizmo::{GizmoMode, Space, Steps, UvDrag, UvHandle, VolumeDrag, WarpPointDrag, snap_to_surface};
use materialcraft::projection::{Projection, ProjectionMode, SceneFrame, ViewProjector};
use view::{GizmoHit, GizmoView, uv_under};

/// The gizmo's session settings (PJ-23: step sizes and gizmo size are settings).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoSettings {
    pub shown: bool,
    pub mode: GizmoMode,
    pub space: Space,
    pub steps: Steps,
    /// Arm length on screen (UI points).
    pub size: f32,
    /// Warp: the view shows the grid's points to drag instead of the volume gizmo.
    pub warp_points: bool,
}

impl Default for GizmoSettings {
    fn default() -> GizmoSettings {
        GizmoSettings { shown: true, mode: GizmoMode::Move, space: Space::Local, steps: Steps::default(), size: 90.0, warp_points: false }
    }
}

/// The viewport's pointer as the gizmo sees it (view coordinates).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoPointer {
    pub at: [f32; 2],
    pub phase: Phase,
    /// Ctrl: move, turn and scale in steps.
    pub step: bool,
    /// Shift: snap onto the surface (centre handle of planar and warp) or lock a UV move to one axis.
    pub shift: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Press,
    Drag,
    Release,
}

/// What one pointer event did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GizmoOutcome {
    /// The document changed (the set needs recompositing).
    pub changed: bool,
    /// A drag ended (recomposite once more at full rate).
    pub ended: bool,
}

/// A drag in progress, with the history key all its edits are recorded under.
#[derive(Debug, Clone)]
struct ActiveDrag {
    target: FillRef,
    key: CoalesceKey,
    kind: DragKind,
}

#[derive(Debug, Clone)]
enum DragKind {
    Volume(VolumeDrag),
    /// The volume dragged over the surface; it started as this projection.
    Surface(Projection),
    Uv {
        drag: UvDrag,
        material: usize,
    },
    WarpPoint(WarpPointDrag),
}

/// The session's projection editing state.
#[derive(Default)]
pub struct ProjectionEditing {
    pub settings: GizmoSettings,
    drag: Option<ActiveDrag>,
    next_key: u64,
    /// The projection panel's running slider drag.
    panel_key: Option<CoalesceKey>,
    /// The scene frame volumes are placed in, by mesh revision.
    frame: Option<(u64, Option<SceneFrame>)>,
    /// When a drag last recomposited the set (drags recomposite a few times a second).
    pub refreshed: Option<std::time::Instant>,
}

/// What the gizmo works with for one pointer event.
pub struct GizmoFrame<'a> {
    pub view: &'a dyn ViewProjector,
    pub frame: Option<&'a SceneFrame>,
    pub surface: Option<&'a SurfaceMap>,
}

impl ProjectionEditing {
    /// A fresh history key.
    fn key(&mut self) -> CoalesceKey {
        self.next_key = self.next_key.wrapping_add(1);
        CoalesceKey(self.next_key)
    }

    /// The frame of the whole scene (as the compositor places volumes in it), computed once per mesh revision.
    pub fn scene_frame(&mut self, mesh: &SharedMesh) -> Option<SceneFrame> {
        match self.frame {
            Some((revision, frame)) if revision == mesh.revision => frame,
            _ => {
                let frame = SceneFrame::of_mesh(&mesh.mesh);
                self.frame = Some((mesh.revision, frame));
                frame
            }
        }
    }

    /// True while a gizmo drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// The fill being dragged, if any.
    pub fn dragged(&self) -> Option<&FillRef> {
        self.drag.as_ref().map(|d| &d.target)
    }

    /// Forgets a drag in progress (the document changed under it); its edits stay as one step.
    pub fn cancel(&mut self, doc: Option<&mut OpenDocument>) {
        if self.drag.take().is_some()
            && let Some(doc) = doc
        {
            doc.close_step();
        }
    }

    /// The gizmo of `target` and what lies under `at`.
    pub fn hover(&self, doc: &OpenDocument, target: &FillRef, f: &GizmoFrame, at: [f32; 2]) -> Option<GizmoHit> {
        GizmoView::build(doc.doc(), target, f.view, f.frame, f.surface, &self.settings)?.hit(at, f.view, doc.doc())
    }

    /// One pointer event on `target`'s gizmo: a press grabs the part under it, drags change the projection (all
    /// under one history key), a release ends the step.
    pub fn pointer(&mut self, doc: &mut OpenDocument, target: &FillRef, f: &GizmoFrame, p: GizmoPointer) -> Result<GizmoOutcome, EditError> {
        match p.phase {
            Phase::Press => {
                self.cancel(Some(doc));
                self.drag = self.grab(doc, target, f, p);
                Ok(GizmoOutcome::default())
            }
            Phase::Drag => self.drag_to(doc, f, p),
            Phase::Release => {
                let out = self.drag_to(doc, f, p);
                let ended = self.drag.is_some();
                self.cancel(Some(doc));
                out.map(|o| GizmoOutcome { ended, ..o })
            }
        }
    }

    fn grab(&mut self, doc: &OpenDocument, target: &FillRef, f: &GizmoFrame, p: GizmoPointer) -> Option<ActiveDrag> {
        let gizmo = GizmoView::build(doc.doc(), target, f.view, f.frame, f.surface, &self.settings)?;
        let hit = gizmo.hit(p.at, f.view, doc.doc())?;
        let projection = target.fill(doc.doc())?.projection.clone();
        let kind = match (hit, &gizmo) {
            (GizmoHit::Volume(handle), GizmoView::Volume(v)) => {
                if p.shift && v.snaps && handle == materialcraft::projection::gizmo::Handle::MoveView {
                    DragKind::Surface(projection)
                } else {
                    let ray = f.view.ray(p.at)?;
                    let volume = projection.mode.volume()?;
                    DragKind::Volume(VolumeDrag::begin(handle, self.settings.space, volume, f.frame?, ray, ray.dir, v.gizmo.length)?)
                }
            }
            (GizmoHit::Uv(handle), GizmoView::Uv(v)) => {
                let from = uv_under(f.view, doc.doc(), v.material, p.at)?;
                DragKind::Uv { drag: UvDrag::begin(handle, &projection.image, from, None), material: v.material }
            }
            (GizmoHit::WarpPoint(index), GizmoView::WarpGrid(_)) => {
                let ProjectionMode::Warp(warp) = &projection.mode else { return None };
                DragKind::WarpPoint(WarpPointDrag::begin(warp, f.frame?, index, f.view.ray(p.at)?)?)
            }
            _ => return None,
        };
        Some(ActiveDrag { target: target.clone(), key: self.key(), kind })
    }

    fn drag_to(&mut self, doc: &mut OpenDocument, f: &GizmoFrame, p: GizmoPointer) -> Result<GizmoOutcome, EditError> {
        let Some(drag) = &self.drag else { return Ok(GizmoOutcome::default()) };
        let Some(current) = drag.target.fill(doc.doc()).map(|fill| fill.projection.clone()) else {
            self.drag = None;
            return Ok(GizmoOutcome::default());
        };
        let steps = p.step.then_some(&self.settings.steps);
        let mut next = current.clone();
        let label = match &drag.kind {
            DragKind::Volume(d) => {
                let (Some(ray), Some(volume)) = (f.view.ray(p.at), next.mode.volume_mut()) else { return Ok(GizmoOutcome::default()) };
                *volume = d.update(ray, steps);
                "Place projection"
            }
            DragKind::Surface(start) => {
                let (Some(hit), Some(frame), Some(volume)) = (f.view.pick(p.at), f.frame, start.mode.volume()) else { return Ok(GizmoOutcome::default()) };
                let snapped = snap_to_surface(volume, frame, hit.position, hit.normal);
                if let Some(v) = next.mode.volume_mut() {
                    *v = snapped;
                }
                "Snap projection to surface"
            }
            DragKind::Uv { drag: d, material } => {
                let Some(uv) = uv_under(f.view, doc.doc(), *material, p.at) else { return Ok(GizmoOutcome::default()) };
                let step = (p.step && d.handle == UvHandle::Rotate).then_some(self.settings.steps.angle);
                next.image = d.update(uv, p.shift, step);
                "Place image"
            }
            DragKind::WarpPoint(d) => {
                let (Some(grid), ProjectionMode::Warp(warp)) = (f.view.ray(p.at).and_then(|ray| d.update(ray)), &mut next.mode) else {
                    return Ok(GizmoOutcome::default());
                };
                warp.grid = grid;
                "Move warp point"
            }
        };
        if next == current {
            return Ok(GizmoOutcome::default());
        }
        let (target, key) = (drag.target.clone(), drag.key);
        doc.edit(label, Some(key), |tx| set_projection(tx, &target, next))?;
        Ok(GizmoOutcome { changed: true, ended: false })
    }

    /// A projection panel change of `target`: with `gesture` (a slider being dragged) it joins the running
    /// gesture's history step.
    pub fn set_from_panel(&mut self, doc: &mut OpenDocument, target: &FillRef, projection: Projection, gesture: bool) -> Result<(), EditError> {
        let key = match (gesture, self.panel_key) {
            (false, _) => None,
            (true, Some(k)) => Some(k),
            (true, None) => {
                let k = self.key();
                self.panel_key = Some(k);
                Some(k)
            }
        };
        doc.edit("Projection", key, |tx| set_projection(tx, target, projection))
    }

    /// The panel's gesture ended: the next change starts a new step.
    pub fn end_panel_gesture(&mut self, doc: Option<&mut OpenDocument>) {
        if self.panel_key.take().is_some()
            && let Some(doc) = doc
        {
            doc.close_step();
        }
    }
}
