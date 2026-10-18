//! The polygon fill tool (PT-45..PT-49): a click picks the triangle under the pointer, a drag a rectangle or a
//! lasso; the picks grow by the pick mode and go into the selected target's polygon fill effect (white fills,
//! black un-picks), one history step per pick. The picking itself is `materialcraft::polygon_pick`: through the 3D
//! view's camera, or in UV space with `uv_pick` for gestures in the 2D view (a click outside the 0–1 working area
//! picks nothing, V2-05).

use super::super::input::StrokeInput;
use super::super::view2d::{UvMap, in_working_area};
use super::{PaintFrame, PaintReport, Painting};
use materialcraft::mesh::Mesh;
use materialcraft::paint::edit::StackId;
use materialcraft::paint::edit::ops::{PolygonFillChange, edit_polygon_fill};
use materialcraft::paint::effect::PickMode;
use materialcraft::polygon_pick::{MeshTopologies, PickRegion, ScreenPick, apply_pick, name_picks, uv_pick};
use materialcraft_brush::TargetPart;

/// A drag shorter than this (view points) is a click.
const CLICK: f32 = 3.0;

/// The shape a drag picks with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DragShape {
    #[default]
    Rectangle,
    Lasso,
}

/// The polygon fill tool's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PolygonFillSettings {
    pub mode: PickMode,
    pub shape: DragShape,
    /// A drag picks only what the camera sees (not through the model).
    pub visible_only: bool,
    /// White fills the picks; black takes them out (the swap key).
    pub white: bool,
}

impl Default for PolygonFillSettings {
    fn default() -> PolygonFillSettings {
        PolygonFillSettings { mode: PickMode::Triangle, shape: DragShape::Rectangle, visible_only: true, white: true }
    }
}

/// The pick gesture in progress (view coordinates).
#[derive(Default)]
pub struct PickGesture {
    path: Vec<[f32; 2]>,
}

impl PickGesture {
    /// The region the gesture marked so far, with `shape` for a drag.
    fn region(&self, shape: DragShape) -> Option<PickRegion> {
        let (first, last) = (*self.path.first()?, *self.path.last()?);
        let extent = self.path.iter().map(|p| (p[0] - first[0]).abs().max((p[1] - first[1]).abs())).fold(0.0f32, f32::max);
        Some(match (extent < CLICK, shape) {
            (true, _) => PickRegion::Point(first),
            (false, DragShape::Rectangle) => PickRegion::Rect(first, last),
            (false, DragShape::Lasso) => PickRegion::Lasso(self.path.clone()),
        })
    }

    /// The outline to draw while dragging.
    pub fn outline(&self, shape: DragShape) -> Vec<[f32; 2]> {
        match self.region(shape) {
            Some(PickRegion::Rect(a, b)) => vec![a, [b[0], a[1]], b, [a[0], b[1]], a],
            Some(PickRegion::Lasso(mut p)) => {
                if let Some(first) = p.first().copied() {
                    p.push(first);
                }
                p
            }
            _ => Vec::new(),
        }
    }
}

/// The topologies of the open mesh for one UV set, kept until the mesh changes.
#[derive(Default)]
pub struct TopologyCache {
    key: Option<(usize, usize, usize)>,
    topologies: Option<MeshTopologies>,
}

impl TopologyCache {
    fn get(&mut self, mesh: &Mesh, uv_set: usize) -> &MeshTopologies {
        let id = mesh.parts.first().map_or(0, |p| p.positions.as_ptr() as usize);
        let key = Some((id, mesh.parts.iter().map(|p| p.triangles.len()).sum(), uv_set));
        if self.key != key {
            self.key = key;
            self.topologies = None;
        }
        self.topologies.get_or_insert_with(|| MeshTopologies::new(mesh, uv_set))
    }
}

impl Painting {
    /// This frame's input to the polygon fill tool.
    pub(super) fn polygon_frame(&mut self, input: Vec<StrokeInput>, mut f: PaintFrame) -> PaintReport {
        let mut report = PaintReport::default();
        for ev in input {
            match ev {
                StrokeInput::Down(at) => self.gesture = Some(PickGesture { path: vec![at.pos] }),
                StrokeInput::Move(at) => {
                    if let Some(g) = &mut self.gesture {
                        g.path.push(at.pos);
                    }
                }
                StrokeInput::Line { to, .. } => self.gesture = Some(PickGesture { path: vec![to.pos] }),
                StrokeInput::Up => {
                    let Some(g) = self.gesture.take() else { continue };
                    let Some(region) = g.region(self.brush.polygon.shape) else { continue };
                    self.pick(&mut f, region, &mut report);
                }
            }
        }
        report.busy = self.gesture.is_some();
        report
    }

    /// Picks `region` into the selected target: one history step.
    fn pick(&mut self, f: &mut PaintFrame, region: PickRegion, report: &mut PaintReport) {
        let Some((target, set)) = f.target.clone() else { return };
        let (doc, picking) = (&mut *f.doc, f.picking);
        let settings = self.brush.polygon;
        let named = {
            let d = doc.doc();
            let Some(set_ref) = d.texture_sets.get(set) else { return };
            let picked = match (f.uv, f.camera) {
                (Some(map), _) => match region_in_uv(&region, map) {
                    PickRegion::Point(p) if !in_working_area(p) => return,
                    uv => uv_pick(&d.mesh, set_ref.material_slot, set_ref.uv_set, &uv),
                },
                (None, Some(camera)) => {
                    let Some(projector) = picking.projector_on(&d.mesh, camera) else {
                        report.message = Some("The 3D view is not ready for picking yet.".into());
                        return;
                    };
                    let shown = |p: usize| picking.is_shown(p);
                    let material = set_ref.material_slot;
                    ScreenPick { projector: &projector, mesh: &d.mesh, material, visible_only: settings.visible_only, shown: &shown }.pick(&region)
                }
                (None, None) => return,
            };
            let grown = self.topologies.get(&d.mesh, set_ref.uv_set).grow(&d.mesh, settings.mode, picked);
            name_picks(&d.mesh, &grown)
        };
        if named.is_empty() {
            return;
        }
        let stack = match target.part {
            TargetPart::Mask => StackId::Mask,
            TargetPart::Content => StackId::Content,
        };
        let label = if settings.white { "Polygon fill" } else { "Polygon un-pick" };
        let result = doc.edit(label, None, |tx| edit_polygon_fill(tx, &target.layer, stack, |s| apply_pick(s, &named, settings.white, settings.mode)));
        match result {
            Ok(PolygonFillChange::Unchanged) => {}
            Ok(_) => report.refresh = Some(set),
            Err(e) => report.message = Some(format!("The polygon fill could not change: {e}.")),
        }
    }
}

/// A region marked in the 2D view (view coordinates) in UV units.
fn region_in_uv(region: &PickRegion, map: UvMap) -> PickRegion {
    match region {
        PickRegion::Point(p) => PickRegion::Point(map.uv(*p)),
        PickRegion::Rect(a, b) => PickRegion::Rect(map.uv(*a), map.uv(*b)),
        PickRegion::Lasso(points) => PickRegion::Lasso(points.iter().map(|p| map.uv(*p)).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_paint::view2d::View2d;

    #[test]
    fn regions_from_the_2d_view_become_uv() {
        let map = UvMap { view: View2d { centre: [0.5, 0.5], zoom: 200.0 }, size: [200.0, 200.0] };
        assert_eq!(region_in_uv(&PickRegion::Point([50.0, 150.0]), map), PickRegion::Point([0.25, 0.75]));
        assert_eq!(region_in_uv(&PickRegion::Rect([0.0, 0.0], [100.0, 25.0]), map), PickRegion::Rect([0.0, 0.0], [0.5, 0.125]));
    }

    #[test]
    fn a_short_drag_is_a_click_and_a_long_one_a_shape() {
        let jitter = PickGesture { path: vec![[10.0, 10.0], [11.0, 12.0], [10.5, 9.0]] };
        assert_eq!(jitter.region(DragShape::Lasso), Some(PickRegion::Point([10.0, 10.0])));
        let drag = PickGesture { path: vec![[10.0, 10.0], [30.0, 15.0], [40.0, 50.0]] };
        assert_eq!(drag.region(DragShape::Rectangle), Some(PickRegion::Rect([10.0, 10.0], [40.0, 50.0])));
        assert_eq!(drag.region(DragShape::Lasso), Some(PickRegion::Lasso(drag.path.clone())));
        assert_eq!(drag.outline(DragShape::Rectangle).len(), 5, "closed");
        assert_eq!(drag.outline(DragShape::Lasso).last(), Some(&[10.0, 10.0]));
        assert_eq!(PickGesture::default().region(DragShape::Rectangle), None);
    }
}
