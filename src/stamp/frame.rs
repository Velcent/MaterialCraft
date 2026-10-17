//! The texel → dab-frame mapping of one dab for 3D-view strokes (PT-08): where a texel's surface point lies in the
//! dab's flat frame (screen units), per placement mode. The pull design asks this for every texel near the dab.

use super::index::Aabb;
use crate::mesh::Vec3;
use crate::paint::brush::Placement;
use crate::projection::ViewProjector;
use crate::v3::{cross, dot, len, mul, norm, sub};

/// Screen units per object unit at object-space point `pos`, for a step across the view (no foreshortening).
pub fn screen_scale_at(projector: &dyn ViewProjector, pos: Vec3) -> Option<f32> {
    let right = camera_axes(projector).0;
    let s0 = projector.to_screen(pos)?.pos;
    let e = pos.iter().fold(1.0f32, |m, v| m.max(v.abs())) * 1e-3;
    let s1 = projector.to_screen(crate::v3::add(pos, mul(right, e)))?.pos;
    let k = ((s1[0] - s0[0]).powi(2) + (s1[1] - s0[1]).powi(2)).sqrt() / e;
    (k.is_finite() && k > 0.0).then_some(k)
}

/// The camera's right and up directions (world = object directions: the fit only moves and scales uniformly).
fn camera_axes(projector: &dyn ViewProjector) -> (Vec3, Vec3) {
    let v = &projector.camera().view;
    (norm([v[0], v[4], v[8]]), norm([v[1], v[5], v[9]]))
}

/// A dab laid on the surface at its centre's hit (wrapped and flat placement).
#[derive(Debug, Clone, Copy)]
pub(super) struct Anchor {
    pub pos: Vec3,
    pub normal: Vec3,
    /// Tangent axes: `u` along the screen's right, `v` along its up.
    pub u: Vec3,
    pub v: Vec3,
    /// Screen units per object unit at the anchor.
    pub scale: f32,
    /// How far (object units) the footprint reaches from the centre.
    pub reach: f32,
}

/// The mapping of one dab.
#[derive(Debug, Clone, Copy)]
pub(super) enum DabFrame {
    /// The texel's screen position is its frame position.
    Screen,
    /// Laid on the surface around `anchor`; `flat` lies in the tangent plane and fades off it, otherwise the stamp
    /// wraps (offsets keep their length along the surface).
    Surface { center: [f32; 2], anchor: Anchor, flat: bool },
}

impl DabFrame {
    /// The frame for a dab centred at screen `center` reaching `reach_px` screen units. Surface placements whose
    /// centre misses the mesh fall back to the screen mapping.
    pub fn new(projector: &dyn ViewProjector, placement: Placement, center: [f32; 2], reach_px: f32) -> DabFrame {
        let flat = match placement {
            Placement::FacingView | Placement::UvAligned => return DabFrame::Screen,
            Placement::Wrapped => false,
            Placement::Flat => true,
        };
        let Some(hit) = projector.pick(center) else { return DabFrame::Screen };
        let Some(scale) = screen_scale_at(projector, hit.position) else { return DabFrame::Screen };
        let n = norm(hit.normal);
        let (right, up) = camera_axes(projector);
        let along = |a: Vec3| sub(a, mul(n, dot(a, n)));
        let u = if len(along(right)) > 1e-3 { norm(along(right)) } else { norm(cross(up, n)) };
        let v = norm(cross(n, u));
        let anchor = Anchor { pos: hit.position, normal: n, u, v, scale, reach: reach_px / scale };
        DabFrame::Surface { center, anchor, flat }
    }

    /// Whether a cell with bounds `b` (already in the copy's space) may hold texels under the dab, given the dab's
    /// frame bounds.
    pub fn may_touch(&self, projector: &dyn ViewProjector, b: &Aabb, bounds: [f32; 4]) -> bool {
        match self {
            DabFrame::Screen => {
                let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
                for c in b.corners() {
                    // a corner behind the camera: the box may reach anywhere on screen
                    let Some(s) = projector.to_screen(c) else { return true };
                    r = [r[0].min(s.pos[0]), r[1].min(s.pos[1]), r[2].max(s.pos[0]), r[3].max(s.pos[1])];
                }
                r[0] <= bounds[2] && r[2] >= bounds[0] && r[1] <= bounds[3] && r[3] >= bounds[1]
            }
            DabFrame::Surface { anchor, .. } => b.distance_squared(anchor.pos) <= anchor.reach * anchor.reach,
        }
    }

    /// Frame position and a fade factor of a surface point at `pos` (object space) shown at `screen`; `None` where
    /// the dab cannot reach it.
    #[inline]
    pub fn map(&self, pos: Vec3, screen: [f32; 2]) -> Option<([f32; 2], f32)> {
        match self {
            DabFrame::Screen => Some((screen, 1.0)),
            DabFrame::Surface { center, anchor, flat } => {
                let d = sub(pos, anchor.pos);
                let off_plane = dot(d, anchor.normal);
                if off_plane.abs() > anchor.reach {
                    return None;
                }
                let (a, b) = (dot(d, anchor.u), dot(d, anchor.v));
                let (a, b, fade) = if *flat {
                    (a, b, 1.0 - off_plane.abs() / anchor.reach.max(1e-12))
                } else {
                    // wrapped: keep the offset's full length (chord ≈ arc), in the tangent direction
                    let t = (a * a + b * b).sqrt();
                    let l = len(d);
                    if t > 1e-12 { (a * l / t, b * l / t, 1.0) } else { (a, b, 1.0) }
                };
                Some(([center[0] + a * anchor.scale, center[1] - b * anchor.scale], fade))
            }
        }
    }
}
