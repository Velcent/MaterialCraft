use super::*;
use crate::v3::{cross, dot, len, norm, sub};

const VIEWPORT: [f32; 2] = [1600.0, 900.0];

fn close(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
    len(sub(a, b)) <= eps
}

/// A camera looking from `eye` at `target`, turned by `roll` about its viewing axis.
fn scene(eye: Vec3, target: Vec3, roll: f32, lens: SceneLens) -> SceneCamera {
    let f = norm(sub(target, eye));
    let level_right = norm(cross(f, [0.0, 1.0, 0.0]));
    let level_up = cross(level_right, f);
    let (s, c) = roll.sin_cos();
    let up = norm([level_up[0] * c - level_right[0] * s, level_up[1] * c - level_right[1] * s, level_up[2] * c - level_right[2] * s]);
    SceneCamera { name: "cam".into(), eye, forward: f, up, lens, aspect: None }
}

#[test]
fn any_pose_becomes_an_orbit_with_the_same_eye_and_axes() {
    let poses = [
        ([3.0, 2.0, 1.5], [0.0, 0.0, 0.0], 0.0),
        ([3.0, 2.0, 1.5], [0.4, -0.2, 0.1], 0.3),
        ([-0.5, 0.2, -4.0], [0.6, 0.3, 0.0], -0.17),
        // pivot plane behind the eye: looking away from the origin
        ([0.2, 0.1, 0.3], [1.0, 0.5, 2.0], 0.05),
        // straight down and straight up
        ([0.1, 5.0, 0.2], [0.1, 0.0, 0.2], 0.0),
        ([0.0, -3.0, 0.0], [0.0, 0.0, 0.0], 1.0),
    ];
    for (eye, target, roll) in poses {
        let s = scene(eye, target, roll, SceneLens::Perspective { fov_y: 0.8 });
        let o = Orbit::from_pose(s.eye, s.forward, s.up).unwrap();
        let b = o.basis();
        assert!(close(b.eye, s.eye, 1e-4), "{eye:?}: {:?}", b.eye);
        assert!(close(b.forward, s.forward, 2e-4), "{eye:?}: {:?} vs {:?}", b.forward, s.forward);
        assert!(close(b.up, s.up, 2e-4), "{eye:?}: {:?} vs {:?}", b.up, s.up);
        // the camera drawn from the orbit projects the target where the scene camera does: the centre
        let (orbit, settings) = s.view().unwrap();
        let p = settings.camera(&orbit, VIEWPORT).project(target).unwrap();
        assert!((p.screen[0] - 800.0).abs() < 0.05 && (p.screen[1] - 450.0).abs() < 0.05, "{p:?}");
    }
    assert_eq!(Orbit::from_pose([0.0; 3], [0.0; 3], [0.0, 1.0, 0.0]), None);
    assert_eq!(Orbit::from_pose([0.0; 3], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0]), None);
    assert_eq!(Orbit::from_pose([f32::NAN, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]), None);
}

#[test]
fn a_level_orbit_round_trips_without_roll() {
    let o = Orbit { yaw: 0.8, pitch: 0.35, distance: 2.7, pan: [0.2, -0.1], roll: 0.0 };
    let b = o.basis();
    let back = Orbit::from_pose(b.eye, b.forward, b.up).unwrap();
    for (a, b) in [(back.yaw, o.yaw), (back.pitch, o.pitch), (back.distance, o.distance), (back.pan[0], o.pan[0]), (back.pan[1], o.pan[1])] {
        assert!((a - b).abs() < 1e-5, "{back:?}");
    }
    assert!(back.roll.abs() < 1e-5);
    // a roll turns the image: the right axis tilts up by the roll angle
    let rolled = Orbit { roll: 0.25, ..o }.basis();
    assert!((dot(rolled.right, b.up) - 0.25f32.sin()).abs() < 1e-5);
    assert!(close(rolled.forward, b.forward, 1e-6));
}

#[test]
fn rotations_round_trip_and_turn_the_camera_axes() {
    // 90° about +y: the camera looks down −x
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let c = SceneCamera::from_rotation("c", [1.0, 2.0, 3.0], [0.0, h, 0.0, h], SceneLens::Perspective { fov_y: 0.5 }, Some(1.5)).unwrap();
    assert!(close(c.forward, [-1.0, 0.0, 0.0], 1e-6) && close(c.up, [0.0, 1.0, 0.0], 1e-6));
    for q in [[0.0, h, 0.0, h], [0.207, -0.492, -0.110, -0.838], [0.9, 0.1, -0.3, 0.05], [0.0, 0.0, 1.0, 0.0]] {
        let c = SceneCamera::from_rotation("c", [0.0; 3], q, SceneLens::Perspective { fov_y: 0.5 }, None).unwrap();
        let back = SceneCamera::from_rotation("c", [0.0; 3], c.rotation(), c.lens, None).unwrap();
        assert!(back.close_to(&c, 1e-5), "{q:?}");
    }
    assert!(SceneCamera::from_rotation("c", [0.0; 3], [0.0; 4], SceneLens::Perspective { fov_y: 0.5 }, None).is_none());
}

#[test]
fn placing_and_orthographic_views_keep_the_framing() {
    let s = scene([0.0, 4.0, 10.0], [0.0, 4.0, 0.0], 0.0, SceneLens::Orthographic { height: 6.0 });
    // the fit moves the model's centre (0, 4, 0) to the origin and halves it
    let placed = s.placed([0.0, -4.0, 0.0], 0.5);
    assert!(close(placed.eye, [0.0, 0.0, 5.0], 1e-6));
    assert_eq!(placed.lens, SceneLens::Orthographic { height: 3.0 });
    let (orbit, settings) = placed.view().unwrap();
    assert_eq!(settings.projection, Projection::Orthographic);
    let cam = settings.camera(&orbit, VIEWPORT);
    // the frame is 3 units tall: a point 1.5 above the centre lands on the top edge
    let top = cam.project([0.0, 1.5, 0.0]).unwrap();
    assert!((top.screen[1]).abs() < 0.05 && (top.screen[0] - 800.0).abs() < 0.05, "{top:?}");
    assert!(cam.project([0.0, 0.0, 0.99]).is_some_and(|p| p.view_depth > CameraSettings::NEAR), "the model lies beyond the near plane");
    // and back
    let again = SceneCamera::of_view("cam", &orbit, &settings, None);
    assert!(matches!(again.lens, SceneLens::Orthographic { height } if (height - 3.0).abs() < 1e-4));
    assert!(close(again.forward, placed.forward, 1e-6));
}
