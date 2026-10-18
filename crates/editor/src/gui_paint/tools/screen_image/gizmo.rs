//! The one transform gizmo of the stencil and the projection image (ADR 0008 mc2-q-pt-05c): with the stencil key
//! held, a drag moves, turns or scales the image laid over the view; turning snaps to 90° steps on request.
//! Plain maths on [`ScreenImageTransform`], so it is tested without a window.

use materialcraft::projection::ScreenImageTransform;

/// Rotation snap step (degrees).
pub const SNAP_DEGREES: f32 = 90.0;

/// Scale change per view unit dragged to the right (exponential, so left and right undo each other).
const SCALE_PER_UNIT: f32 = 0.01;

/// What a gizmo drag does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GizmoAction {
    Move,
    /// Turn about the image's centre, following the pointer around it.
    Turn,
    /// Scale about the image's centre: drag right to grow, left to shrink.
    Scale,
}

/// One gizmo drag: the placement it started from and where.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GizmoDrag {
    pub action: GizmoAction,
    start: ScreenImageTransform,
    from: [f32; 2],
}

impl GizmoDrag {
    pub fn begin(action: GizmoAction, start: ScreenImageTransform, from: [f32; 2]) -> GizmoDrag {
        GizmoDrag { action, start, from }
    }

    /// The placement with the pointer at `to`; `snap` turns in 90° steps (absolute rotation).
    pub fn update(&self, to: [f32; 2], snap: bool) -> ScreenImageTransform {
        let mut t = self.start;
        let centre = t.centre;
        match self.action {
            GizmoAction::Move => t.translate([to[0] - self.from[0], to[1] - self.from[1]]),
            GizmoAction::Turn => {
                let Some(delta) = turn(centre, self.from, to) else { return t };
                let degrees = if snap {
                    let target = ((t.rotation + delta) / SNAP_DEGREES).round() * SNAP_DEGREES;
                    target - t.rotation
                } else {
                    delta
                };
                t.rotate_about(degrees, centre);
            }
            GizmoAction::Scale => t.scale_about((((to[0] - self.from[0]) * SCALE_PER_UNIT).exp()).clamp(1e-3, 1e3), centre),
        }
        t
    }
}

/// The counter-clockwise screen angle (degrees) from `from` to `to` around `centre`; `None` too close to it.
fn turn(centre: [f32; 2], from: [f32; 2], to: [f32; 2]) -> Option<f32> {
    // screen y points down: flip it to measure counter-clockwise turns
    let angle = |p: [f32; 2]| {
        let (dx, dy) = (p[0] - centre[0], centre[1] - p[1]);
        (dx * dx + dy * dy > 1.0).then(|| dy.atan2(dx).to_degrees())
    };
    let d = angle(to)? - angle(from)?;
    Some((d + 180.0).rem_euclid(360.0) - 180.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::projection::Repeat;

    fn placed() -> ScreenImageTransform {
        ScreenImageTransform { centre: [200.0, 150.0], size: [100.0, 60.0], rotation: 10.0, repeat: Repeat::None }
    }

    fn close(a: [f32; 2], b: [f32; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4
    }

    #[test]
    fn moving_and_turning_keep_the_image_point_under_the_pointer() {
        let from = [230.0, 140.0];
        let under = placed().to_image(from);
        let moved = GizmoDrag::begin(GizmoAction::Move, placed(), from).update([260.0, 190.0], false);
        assert!(close(moved.to_image([260.0, 190.0]), under));
        // a quarter turn counter-clockwise around the centre on screen (y down): (+30, -10) → (-10, -30)
        let turned = GizmoDrag::begin(GizmoAction::Turn, placed(), from).update([190.0, 120.0], false);
        assert!(close(turned.to_image([190.0, 120.0]), under), "{:?} vs {under:?}", turned.to_image([190.0, 120.0]));
        assert!((turned.rotation - 100.0).abs() < 1e-3, "{}", turned.rotation);
        assert_eq!(turned.centre, placed().centre, "turning keeps the centre");
    }

    #[test]
    fn turning_snaps_to_quarter_turns() {
        let d = GizmoDrag::begin(GizmoAction::Turn, placed(), [300.0, 150.0]);
        // 10° start + about 30° pointer turn → 40° → snaps to 0°; + about 70° → 80° → 90°
        let a = d.update([200.0 + 100.0 * 30f32.to_radians().cos(), 150.0 - 100.0 * 30f32.to_radians().sin()], true);
        assert!(a.rotation.abs() < 1e-3 || (a.rotation - 360.0).abs() < 1e-3, "{}", a.rotation);
        let b = d.update([200.0 + 100.0 * 70f32.to_radians().cos(), 150.0 - 100.0 * 70f32.to_radians().sin()], true);
        assert!((b.rotation - 90.0).abs() < 1e-3, "{}", b.rotation);
        // a drag starting on the centre does nothing
        assert_eq!(GizmoDrag::begin(GizmoAction::Turn, placed(), [200.0, 150.0]).update([250.0, 150.0], true), placed());
    }

    #[test]
    fn scaling_grows_to_the_right_about_the_centre_and_undoes_itself() {
        let d = GizmoDrag::begin(GizmoAction::Scale, placed(), [100.0, 100.0]);
        let bigger = d.update([200.0, 100.0], false);
        assert!(bigger.size[0] > 250.0 && bigger.centre == placed().centre);
        let back = GizmoDrag::begin(GizmoAction::Scale, bigger, [200.0, 100.0]).update([100.0, 100.0], false);
        assert!((back.size[0] - 100.0).abs() < 1e-3 && (back.size[1] - 60.0).abs() < 1e-3);
    }
}
