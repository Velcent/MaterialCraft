//! Gizmo maths against the projectors: what a drag does to the volume is what the projection then shows (the
//! image point grabbed follows the pointer), placed volumes round-trip through the stored settings, and the UV
//! handle keeps the image under the pointer.

use super::super::{ImagePlacement, Planar, Projection, ProjectionMode, Projector, SceneFrame, SurfacePoint, UvPlacement, Volume, VolumeTransform};
use super::*;
use crate::projection::rotation::{self, EulerOrder};

/// A scene two units wide around (1, 0, 0) (one scene unit = 2).
const FRAME: SceneFrame = SceneFrame { centre: [1.0, 0.0, 0.0], size: [2.0, 2.0, 1.0] };

fn close3(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

fn close2(a: [f32; 2], b: [f32; 2]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
}

/// A ray looking straight down −z through (x, y).
fn down(x: f32, y: f32) -> Ray {
    Ray::new([x, y, 10.0], [0.0, 0.0, -1.0]).unwrap()
}

const VIEW: [f32; 3] = [0.0, 0.0, -1.0];

fn same_rotation(a: &Volume, b: &Volume) -> bool {
    let m = |v: &Volume| rotation::matrix(v.rotation.map(f32::to_radians), EulerOrder::Xyz);
    m(a).iter().flatten().zip(m(b).iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-4)
}

fn planar(volume: Volume) -> impl Projector {
    Projection { mode: ProjectionMode::Planar(Planar { volume, ..Planar::default() }), ..Projection::default() }.prepare(Some(&FRAME), true).unwrap()
}

fn uv_at(p: &impl Projector, pos: [f32; 3]) -> [f32; 2] {
    let t = p.project(&SurfacePoint { pos, normal: [0.0, 0.0, 1.0], texel_len: 1.0 / 64.0, ..SurfacePoint::default() });
    t.taps[0].uv
}

#[test]
fn placed_volumes_round_trip_and_agree_with_the_projector_frame() {
    let volumes = [
        Volume::default(),
        Volume { offset: [0.3, -0.2, 0.1], rotation: [20.0, -35.0, 110.0], scale: [0.5, 2.0, -0.25], clip: true },
        Volume { offset: [0.0; 3], rotation: [0.0, 90.0, 30.0], scale: [1.0, 1.0, 1.0], clip: false },
    ];
    for v in volumes {
        let placed = PlacedVolume::of(&v, &FRAME);
        let back = placed.to_volume(&FRAME, v.clip);
        assert!(close3(back.offset, v.offset) && close3(back.scale, v.scale) && back.clip == v.clip, "{v:?} → {back:?}");
        assert!(same_rotation(&back, &v), "{v:?} → {back:?}");
        // the box's local coordinates are the projector's
        let t = VolumeTransform::new(&v, &FRAME);
        for l in [[0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [-0.5, 0.5, 0.5], [0.1, -0.3, 0.25]] {
            assert!(close3(t.local(placed.point(l)), l), "{v:?} {l:?}");
        }
        assert_eq!(placed.edges().len(), 12);
    }
}

#[test]
fn moving_along_an_axis_carries_the_image_with_the_pointer() {
    let v = Volume { scale: [0.5, 0.5, 1.0], ..Volume::default() };
    // grab the x arrow at x = 1.2 (the centre is at x = 1) and drag to x = 1.7
    let drag = VolumeDrag::begin(Handle::MoveAxis(0), Space::Local, &v, &FRAME, down(1.2, 0.05), VIEW, 0.5).unwrap();
    let moved = drag.update(down(1.7, 0.4), None);
    assert!(close3(moved.offset, [0.25, 0.0, 0.0]), "{moved:?}");
    // the projection point that was under the pointer is under it again
    assert!(close2(uv_at(&planar(moved), [1.7, 0.05, 0.0]), uv_at(&planar(v), [1.2, 0.05, 0.0])));
    // in steps of 0.1 scene units (0.2 object units): 0.5 → 0.6 object units
    let stepped = drag.update(down(1.75, 0.0), Some(&Steps { distance: 0.1, ..Steps::default() }));
    assert!(close3(stepped.offset, [0.3, 0.0, 0.0]), "{stepped:?}");
    // an axis pointing at the camera can't be grabbed
    assert!(VolumeDrag::begin(Handle::MoveAxis(2), Space::Local, &v, &FRAME, down(1.0, 0.0), VIEW, 0.5).is_none());
}

#[test]
fn moving_in_a_plane_and_the_view_plane() {
    let v = Volume::default();
    let drag = VolumeDrag::begin(Handle::MovePlane(2), Space::World, &v, &FRAME, down(1.1, 0.1), VIEW, 0.5).unwrap();
    assert!(close3(drag.update(down(1.5, -0.3), None).offset, [0.2, -0.2, 0.0]));
    let drag = VolumeDrag::begin(Handle::MoveView, Space::World, &v, &FRAME, down(1.0, 0.0), VIEW, 0.5).unwrap();
    assert!(close3(drag.update(down(0.6, 0.2), None).offset, [-0.2, 0.1, 0.0]));
    // local axes of a turned volume: moving along its x goes along the object's y
    let turned = Volume { rotation: [0.0, 0.0, 90.0], ..Volume::default() };
    let drag = VolumeDrag::begin(Handle::MoveAxis(0), Space::Local, &turned, &FRAME, down(1.0, 0.2), VIEW, 0.5).unwrap();
    assert!(close3(drag.update(down(1.3, 0.6), None).offset, [0.0, 0.2, 0.0]));
}

#[test]
fn turning_follows_the_pointer_around_the_axis_and_snaps_to_quarter_turns() {
    let v = Volume { scale: [0.5, 0.5, 1.0], ..Volume::default() };
    // a quarter turn about z: from +x of the centre to +y of it
    let drag = VolumeDrag::begin(Handle::RotateAxis(2), Space::Local, &v, &FRAME, down(1.5, 0.0), VIEW, 0.5).unwrap();
    let turned = drag.update(down(1.0, 0.5), None);
    assert!(same_rotation(&turned, &Volume { rotation: [0.0, 0.0, 90.0], ..v }), "{turned:?}");
    // the image point under the pointer stays under it
    assert!(close2(uv_at(&planar(turned), [1.0, 0.3, 0.0]), uv_at(&planar(v), [1.3, 0.0, 0.0])));
    // 70° snaps to 90°, 30° to 0° (default steps: quarter turns)
    let a = 70f32.to_radians();
    let snapped = drag.update(down(1.0 + 0.5 * a.cos(), 0.5 * a.sin()), Some(&Steps::default()));
    assert!(same_rotation(&snapped, &Volume { rotation: [0.0, 0.0, 90.0], ..v }), "{snapped:?}");
    let a = 30f32.to_radians();
    assert!(same_rotation(&drag.update(down(1.0 + 0.5 * a.cos(), 0.5 * a.sin()), Some(&Steps::default())), &v));
}

#[test]
fn local_and_world_turns_differ_on_a_turned_volume() {
    let v = Volume { rotation: [90.0, 0.0, 0.0], ..Volume::default() };
    let world = VolumeDrag::begin(Handle::RotateAxis(2), Space::World, &v, &FRAME, down(1.5, 0.0), VIEW, 0.5).unwrap().update(down(1.0, 0.5), None);
    let expected = rotation::mul(&rotation::about(2, std::f32::consts::FRAC_PI_2), &rotation::about(0, std::f32::consts::FRAC_PI_2));
    let got = rotation::matrix(world.rotation.map(f32::to_radians), EulerOrder::Xyz);
    assert!(got.iter().flatten().zip(expected.iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-4), "{world:?}");
    // the volume's own z points along the object's −y: seen from above it can only be turned through a plane hit
    let drag = VolumeDrag::begin(Handle::RotateAxis(2), Space::Local, &v, &FRAME, down(1.5, 0.0), VIEW, 0.5);
    assert!(drag.is_none() || drag.is_some_and(|d| d.update(down(1.4, 0.1), None).rotation.iter().all(|a| a.is_finite())));
}

#[test]
fn free_turns_keep_the_trackball_point_under_the_pointer() {
    let v = Volume::default();
    let drag = VolumeDrag::begin(Handle::RotateFree, Space::World, &v, &FRAME, down(1.0, 0.0), VIEW, 0.5).unwrap();
    // from the top of the sphere towards +x: a turn about +y
    let t = drag.update(down(1.0 + 0.5 * 0.5f32.sin(), 0.0), None);
    let m = rotation::matrix(t.rotation.map(f32::to_radians), EulerOrder::Xyz);
    let z = rotation::apply(&m, [0.0, 0.0, 1.0]);
    assert!(close3(z, [0.5f32.sin(), 0.0, 0.5f32.cos()]), "{z:?}");
}

#[test]
fn scaling_an_axis_follows_the_pointer_and_steps() {
    let v = Volume::default();
    let drag = VolumeDrag::begin(Handle::ScaleAxis(0), Space::World, &v, &FRAME, down(1.5, 0.0), VIEW, 0.5).unwrap();
    assert!(close3(drag.update(down(2.0, 0.3), None).scale, [2.0, 1.0, 1.0]));
    assert!(close3(drag.update(down(1.6, 0.0), Some(&Steps { scale: 0.5, ..Steps::default() })).scale, [1.0, 1.0, 1.0]));
    // shrinking past the centre stops at a thousandth
    assert!(drag.update(down(0.0, 0.0), None).scale[0] > 0.0);
    let uniform = VolumeDrag::begin(Handle::ScaleUniform, Space::World, &v, &FRAME, down(1.2, 0.0), VIEW, 0.5).unwrap();
    assert!(close3(uniform.update(down(1.0, 0.1), None).scale, [0.5, 0.5, 0.5]));
    // a mirrored axis stays mirrored
    let m = Volume { scale: [-1.0, 1.0, 1.0], ..Volume::default() };
    let drag = VolumeDrag::begin(Handle::ScaleAxis(0), Space::Local, &m, &FRAME, down(0.5, 0.0), VIEW, 0.5).unwrap();
    assert!(close3(drag.update(down(0.0, 0.0), None).scale, [-2.0, 1.0, 1.0]));
}

#[test]
fn snapping_to_the_surface_projects_the_image_centre_onto_the_point() {
    let mut decal = Planar { volume: Volume { scale: [0.2, 0.2, 0.2], ..Volume::default() }, ..Planar::default() };
    decal.facing_falloff.on = true;
    for (pos, normal) in [([1.5, 0.2, 0.3], [0.0, 0.0, 1.0]), ([0.2, -0.4, 0.1], [1.0, 1.0, 0.0]), ([1.0, 0.0, -0.4], [0.0, 0.0, -1.0])] {
        let snapped = snap_to_surface(&decal.volume, &FRAME, pos, normal);
        let p = Projection { mode: ProjectionMode::Planar(Planar { volume: snapped, ..decal }), ..Projection::default() }.prepare(Some(&FRAME), true).unwrap();
        let t = p.project(&SurfacePoint { pos, normal, texel_len: 1.0 / 64.0, ..SurfacePoint::default() });
        assert!(close2(t.taps[0].uv, [0.5, 0.5]) && t.coverage > 0.99, "{pos:?} {normal:?}: {t:?}");
        // the volume looks along the normal: its z is the normal
        let placed = PlacedVolume::of(&snapped, &FRAME);
        let n = crate::v3::norm(normal);
        assert!(close3(placed.axis(2), n), "{:?}", placed.axis(2));
        assert!(close3(snapped.scale, decal.volume.scale));
    }
    // a snap with the same normal keeps the turn about it
    let turned = Volume { rotation: [0.0, 0.0, 30.0], ..Volume::default() };
    assert!(same_rotation(&snap_to_surface(&turned, &FRAME, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), &turned));
}

#[test]
fn the_uv_frame_is_one_repetition_of_the_image() {
    let p = ImagePlacement { tiling: [2.0, 4.0], rotation: 30.0, offset: [0.1, -0.2], ..ImagePlacement::default() };
    let (frame, place) = (UvFrame::of(&p), UvPlacement::new(&p));
    assert!(close2(place.apply(frame.centre), [0.5, 0.5]));
    for (f, image) in [([-1.0, -1.0], [0.0, 0.0]), ([1.0, -1.0], [1.0, 0.0]), ([1.0, 1.0], [1.0, 1.0]), ([0.0, 1.0], [0.5, 1.0])] {
        assert!(close2(place.apply(frame.point(f)), image), "{f:?}");
    }
    assert!(frame.contains(frame.point([0.9, -0.9])) && !frame.contains(frame.point([1.1, 0.0])));
    assert_eq!(frame.outline(8).len(), 33);
}

#[test]
fn uv_drags_keep_the_image_under_the_pointer() {
    let p = ImagePlacement { tiling: [2.0, 2.0], rotation: 10.0, offset: [0.05, 0.0], ..ImagePlacement::default() };
    let from = [0.6, 0.45];
    let under = UvPlacement::new(&p).apply(from);
    let moved = UvDrag::begin(UvHandle::Move, &p, from, None).update([0.7, 0.6], false, None);
    assert!(close2(UvPlacement::new(&moved).apply([0.7, 0.6]), under));
    let locked = UvDrag::begin(UvHandle::Move, &p, from, None).update([0.7, 0.48], true, None);
    assert!(close2(locked.offset, [0.15, 0.0]));
    let turned = UvDrag::begin(UvHandle::Rotate, &p, from, None).update([0.5, 0.6], false, None);
    let c = UvFrame::of(&p).centre;
    // the turned pointer lies at the same distance from the centre: the image point follows it
    let r = ((from[0] - c[0]).powi(2) + (from[1] - c[1]).powi(2)).sqrt();
    let a = (0.6f32 - c[1]).atan2(0.5 - c[0]);
    let to = [c[0] + r * a.cos(), c[1] + r * a.sin()];
    let turned_to = UvDrag::begin(UvHandle::Rotate, &p, from, None).update(to, false, None);
    assert!(close2(UvPlacement::new(&turned_to).apply(to), under));
    assert!((turned.rotation - turned_to.rotation).abs() < 1e-3);
    // snapping turns to whole steps; a pivot moves the centre with the turn
    assert!((UvDrag::begin(UvHandle::Rotate, &p, from, None).update(to, false, Some(90.0)).rotation % 90.0).abs() < 1e-3);
    let pivoted = UvDrag::begin(UvHandle::Rotate, &p, [0.5, 0.0], Some([0.0, 0.0])).update([0.0, 0.5], false, None);
    assert!(close2(UvFrame::of(&pivoted).centre, [-c[1], c[0]]), "{pivoted:?}");
}

#[test]
fn uv_scaling_is_symmetric_about_the_centre() {
    let p = ImagePlacement::default();
    // the right edge from u = 1 to u = 0.75: the frame is half as wide, the image repeats twice along u
    let edge = UvDrag::begin(UvHandle::ScaleAxis(0), &p, [1.0, 0.5], None).update([0.75, 0.5], false, None);
    assert!(close2(edge.tiling, [2.0, 1.0]) && close2(edge.offset, [0.0, 0.0]));
    let corner = UvDrag::begin(UvHandle::ScaleUniform, &p, [1.0, 1.0], None).update([1.5, 1.5], false, None);
    assert!(close2(corner.tiling, [0.5, 0.5]));
}

#[test]
fn picking_prefers_knobs_then_areas_then_the_nearest_line() {
    let gizmo = Gizmo::new(&PlacedVolume::of(&Volume::default(), &FRAME), Space::World, GizmoMode::Move, 1.0);
    // looking down −z, 100 screen units per object unit, centre at (200, 200), y down
    let screen = to_screen(&gizmo.shapes(), |p: [f32; 3]| Some([200.0 + 100.0 * (p[0] - 1.0), 200.0 - 100.0 * p[1]]));
    assert_eq!(pick(&screen, [201.0, 199.0], 6.0, 8.0), Some(Handle::MoveView));
    assert_eq!(pick(&screen, [260.0, 202.0], 6.0, 8.0), Some(Handle::MoveAxis(0)));
    assert_eq!(pick(&screen, [200.0, 140.0], 6.0, 8.0), Some(Handle::MoveAxis(1)));
    assert_eq!(pick(&screen, [240.0, 160.0], 6.0, 8.0), Some(Handle::MovePlane(2)));
    assert_eq!(pick(&screen, [300.0, 300.0], 6.0, 8.0), None);
    let rotate = Gizmo { mode: GizmoMode::Rotate, ..gizmo };
    let screen = to_screen(&rotate.shapes(), |p: [f32; 3]| Some([200.0 + 100.0 * (p[0] - 1.0), 200.0 - 100.0 * p[1]]));
    assert_eq!(pick(&screen, [200.0 + 100.0 * 0.6f32.cos(), 200.0 - 100.0 * 0.6f32.sin()], 6.0, 8.0), Some(Handle::RotateAxis(2)));
}
