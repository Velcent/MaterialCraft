//! Straight strokes (PT-31 … PT-33): from the end of the previous stroke to the click, optionally snapped to angle
//! steps. The line is built in the current view; the previous end is kept as a surface point
//! ([`LineAnchor`]) and projected into the view again, so the line starts where the last stroke ended on the
//! model even after the camera moved (ADR 0008 mc2-q-pt-13).

use crate::engine::InputPoint;
use materialcraft::mesh::Vec3;
use materialcraft::projection::ViewProjector;

/// The angle step of a snapped line (PT-32), in degrees.
pub const SNAP_DEGREES: f32 = 5.0;
/// Distance between the points a line is fed to the stroke with (frame units): close enough for model-sized
/// brushes to follow the depth along it.
const STEP: f32 = 4.0;

/// Where the previous stroke ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineAnchor {
    /// On the model: an object-space point, projected into whatever view paints the next line.
    Surface(Vec3),
    /// Off the model (or a 2D-view stroke): a position in the stroke's frame, used as it is.
    Frame([f32; 2]),
}

impl LineAnchor {
    /// The anchor of a stroke that ended at frame position `end`, seen through `projector` (3D view; `None` for a
    /// 2D-view stroke).
    pub fn at(end: [f32; 2], projector: Option<&dyn ViewProjector>) -> LineAnchor {
        match projector.and_then(|p| p.pick(end)) {
            Some(hit) => LineAnchor::Surface(hit.position),
            None => LineAnchor::Frame(end),
        }
    }

    /// Where the line starts in the current view: the surface point projected through `projector`, or the frame
    /// position. `None` when the surface point lies behind the camera.
    pub fn start(&self, projector: Option<&dyn ViewProjector>) -> Option<[f32; 2]> {
        match (self, projector) {
            (LineAnchor::Surface(p), Some(view)) => view.to_screen(*p).map(|s| s.pos),
            (LineAnchor::Surface(_), None) => None,
            (LineAnchor::Frame(f), _) => Some(*f),
        }
    }
}

/// `to` moved onto the nearest ray from `from` at a multiple of `step` degrees, keeping its distance. Multiples of
/// 90° are exact.
pub fn snap_angle(from: [f32; 2], to: [f32; 2], step: f32) -> [f32; 2] {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let l = (dx * dx + dy * dy).sqrt();
    if !(l > 0.0 && step.is_finite() && step > 0.0) {
        return to;
    }
    let degrees = (dy.atan2(dx).to_degrees() / step).round() * step;
    let d = degrees.rem_euclid(360.0);
    let (s, c) = match d {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        _ => d.to_radians().sin_cos(),
    };
    [from[0] + c * l, from[1] + s * l]
}

/// The points of a straight stroke from `from` to `to` with the pen's `pressure`: both ends and points every few
/// frame units between, at increasing times.
pub fn line_points(from: [f32; 2], to: [f32; 2], pressure: f32) -> Vec<InputPoint> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    let l = (dx * dx + dy * dy).sqrt();
    let n = if l.is_finite() { (l / STEP).ceil().clamp(1.0, 100_000.0) as usize } else { 1 };
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let pos = if i == n { to } else { [from[0] + dx * t, from[1] + dy * t] };
            InputPoint { pressure, ..InputPoint::at(pos, i as f64 * 1e-3) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::camera::{Camera, Lens};
    use materialcraft::mesh::{Mesh, MeshPart};
    use materialcraft::projection::ViewScene;

    #[test]
    fn snapping_keeps_the_length_and_lands_on_steps() {
        let flat = snap_angle([10.0, 10.0], [40.0, 11.0], SNAP_DEGREES);
        assert!(flat[1] == 10.0 && (flat[0] - (10.0 + 901f32.sqrt())).abs() < 1e-4, "{flat:?}");
        let p = snap_angle([0.0, 0.0], [10.0, 9.0], SNAP_DEGREES);
        let a = p[1].atan2(p[0]).to_degrees();
        assert!((a - 40.0).abs() < 1e-3 && ((p[0] * p[0] + p[1] * p[1]).sqrt() - 181f32.sqrt()).abs() < 1e-3, "{a}");
        let down = snap_angle([0.0, 0.0], [0.1, -5.0], SNAP_DEGREES);
        assert!(down[0] == 0.0 && (down[1] + 25.01f32.sqrt()).abs() < 1e-5, "{down:?}");
        assert_eq!(snap_angle([1.0, 1.0], [1.0, 1.0], SNAP_DEGREES), [1.0, 1.0]);
    }

    #[test]
    fn lines_run_from_end_to_end() {
        let pts = line_points([0.0, 0.0], [10.0, 0.0], 0.5);
        assert_eq!(pts.first().unwrap().pos, [0.0, 0.0]);
        assert_eq!(pts.last().unwrap().pos, [10.0, 0.0]);
        assert_eq!(pts.len(), 4);
        assert!(pts.iter().all(|p| p.pressure == 0.5) && pts.windows(2).all(|w| w[0].time < w[1].time));
        assert_eq!(line_points([3.0, 3.0], [3.0, 3.0], 1.0).len(), 2);
    }

    #[test]
    fn the_anchor_stays_on_the_surface_when_the_camera_moves() {
        let m = materialcraft::render3d::build_mesh(0, 32, 1.0, None, 0.0, 0.0);
        let mesh = Mesh { materials: vec!["m".into()], parts: vec![MeshPart::from_viewport("sphere", 0, &m)] };
        let scene = ViewScene::new(&mesh, mesh.fit());
        let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
        let cam = |eye: [f32; 3]| Camera::looking_at(eye, [0.0; 3], [0.0, 1.0, 0.0], &lens, [400.0, 300.0]);
        let first = scene.projector(&mesh, cam([0.0, 0.0, 3.0])).unwrap();
        let end = [230.0, 140.0];
        let anchor = LineAnchor::at(end, Some(&first));
        let LineAnchor::Surface(point) = anchor else { panic!("the end is on the sphere") };
        let back = anchor.start(Some(&first)).unwrap();
        assert!((back[0] - end[0]).abs() < 0.05 && (back[1] - end[1]).abs() < 0.05, "{back:?}");
        // after an orbit the start is where the surface point now shows, not the old screen position
        let moved = scene.projector(&mesh, cam([1.2, 0.4, 2.7])).unwrap();
        let start = anchor.start(Some(&moved)).unwrap();
        assert_eq!(start, moved.to_screen(point).unwrap().pos);
        assert!((start[0] - end[0]).abs() > 5.0);
        // off the model the frame position is kept
        assert_eq!(LineAnchor::at([2.0, 2.0], Some(&first)).start(Some(&moved)), Some([2.0, 2.0]));
        assert_eq!(LineAnchor::at([7.0, 8.0], None).start(None), Some([7.0, 8.0]));
    }
}
