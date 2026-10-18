use super::*;

fn close(a: &[f32], b: &[f32], eps: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
}

const VIEWPORT: [f32; 2] = [800.0, 600.0];

#[test]
fn the_orbit_looks_at_its_pivot_from_its_distance() {
    let o = Orbit { yaw: 0.7, pitch: -0.4, distance: 2.5, pan: [0.0, 0.0], roll: 0.0 };
    let c = CameraSettings::default().camera(&o, VIEWPORT);
    let p = c.project([0.0, 0.0, 0.0]).unwrap();
    assert!(close(&p.screen, &[400.0, 300.0], 1e-3), "{p:?}");
    assert!((p.view_depth - 2.5).abs() < 1e-4);
    // the same camera as looking at the pivot
    let lens = CameraSettings::default().lens(o.distance);
    let b = o.basis();
    let l = Camera::looking_at(b.eye, [0.0; 3], [0.0, 1.0, 0.0], &lens, VIEWPORT);
    assert!(close(&c.view, &l.view, 1e-5));
    // a pan moves eye and pivot together: the pivot leaves the centre against the pan
    let panned = Orbit { pan: [0.3, 0.0], ..o };
    let q = CameraSettings::default().camera(&panned, VIEWPORT).project([0.0, 0.0, 0.0]).unwrap();
    assert!(q.screen[0] < 400.0 && (q.screen[1] - 300.0).abs() < 1e-3, "{q:?}");
}

#[test]
fn navigation_stays_in_its_limits_and_snaps_to_axis_views() {
    let mut o = Orbit::default();
    o.rotate(0.0, 10.0);
    assert_eq!(o.pitch, Orbit::PITCH_LIMIT);
    o.zoom(1e-6);
    assert_eq!(o.distance, Orbit::DISTANCE.0);
    o.zoom(1e6);
    assert_eq!(o.distance, Orbit::DISTANCE.1);
    o.zoom(f32::NAN);
    assert_eq!(o.distance, Orbit::DISTANCE.1);
    o.pan_by(-1e6, 0.5);
    assert_eq!(o.pan, [-Orbit::PAN_LIMIT, 0.5]);
    let mut raw = (0.0, 0.0);
    o.rotate_snapped(&mut raw, 0.5, 0.0);
    assert_eq!(o.yaw, 0.0, "half a radian rounds back to the front view");
    o.rotate_snapped(&mut raw, 1.1, 0.1);
    assert!((o.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6 && o.pitch == 0.0);
    assert!((raw.0 - 1.6).abs() < 1e-5, "the raw angle keeps counting");
}

#[test]
fn field_of_view_and_focal_length_stay_linked() {
    let mut s = CameraSettings::default();
    s.set_focal_length(50.0);
    assert!((s.focal_length() - 50.0).abs() < 1e-3);
    s.set_focal_length(1.0);
    assert_eq!(s.fov_y, CameraSettings::FOV_RANGE.1);
    s.set_fov_y(f32::NAN);
    assert_eq!(s.fov_y, CameraSettings::FOV_RANGE.1);
    // 40° is about a 33 mm lens on full frame
    assert!((CameraSettings::default().focal_length() - 32.97).abs() < 0.05);
}

#[test]
fn orthographic_keeps_the_framing_of_the_pivot_plane() {
    let o = Orbit { yaw: 0.0, pitch: 0.0, distance: 3.0, pan: [0.0, 0.0], roll: 0.0 };
    let persp = CameraSettings::default();
    let ortho = CameraSettings { projection: Projection::Orthographic, ..persp };
    let (a, b) = (persp.camera(&o, VIEWPORT), ortho.camera(&o, VIEWPORT));
    assert!(b.is_orthographic() && !a.is_orthographic());
    // a point on the plane through the pivot lands at the same place in both
    let (pa, pb) = (a.project([0.4, 0.3, 0.0]).unwrap(), b.project([0.4, 0.3, 0.0]).unwrap());
    assert!(close(&pa.screen, &pb.screen, 1e-2), "{pa:?} {pb:?}");
    // zooming scales the orthographic view too
    let near = ortho.camera(&Orbit { distance: 1.5, ..o }, VIEWPORT).project([0.4, 0.3, 0.0]).unwrap();
    assert!(near.screen[0] - 400.0 > 1.9 * (pb.screen[0] - 400.0));
}

#[test]
fn a_jittered_camera_moves_every_point_by_the_offset() {
    let o = Orbit::default();
    for s in [CameraSettings::default(), CameraSettings { projection: Projection::Orthographic, ..CameraSettings::default() }] {
        let c = s.camera(&o, VIEWPORT);
        let j = c.jittered([0.25, -0.5], VIEWPORT);
        for p in [[0.0, 0.0, 0.0], [0.5, -0.3, 0.8], [-1.0, 0.2, -0.6]] {
            let (a, b) = (c.project(p).unwrap(), j.project(p).unwrap());
            assert!(close(&[b.screen[0] - a.screen[0], b.screen[1] - a.screen[1]], &[0.25, -0.5], 1e-3), "{a:?} {b:?}");
            assert!((a.depth - b.depth).abs() < 1e-6);
        }
    }
}

#[test]
fn looking_through_the_lens_keeps_the_focus_plane_in_place() {
    let o = Orbit { yaw: 0.3, pitch: 0.2, distance: 3.0, pan: [0.0, 0.0], roll: 0.0 };
    let c = CameraSettings::default().camera(&o, VIEWPORT);
    let shifted = c.through_lens([0.05, -0.03], 3.0);
    // the pivot lies on the focus plane
    let (a, b) = (c.project([0.0; 3]).unwrap(), shifted.project([0.0; 3]).unwrap());
    assert!(close(&a.screen, &b.screen, 1e-2), "{a:?} {b:?}");
    // a point nearer the camera moves
    let near = o.basis().eye.map(|v| v * 0.5);
    let (a, b) = (c.project(near).unwrap(), shifted.project(near).unwrap());
    assert!(!close(&a.screen, &b.screen, 1.0), "{a:?} {b:?}");
    // the eye moved by the offset along the camera's axes
    let e = shifted.eye().unwrap();
    let want = crate::v3::add(o.basis().eye, crate::v3::add(crate::v3::mul(o.basis().right, 0.05), crate::v3::mul(o.basis().up, -0.03)));
    assert!(close(&e, &want, 1e-4), "{e:?} {want:?}");
    // a parallel projection has no lens
    let ortho = CameraSettings { projection: Projection::Orthographic, ..CameraSettings::default() }.camera(&o, VIEWPORT);
    assert_eq!(ortho.through_lens([0.05, 0.0], 3.0), ortho);
}

#[test]
fn sample_sequences_start_at_the_plain_camera_and_stay_in_range() {
    assert_eq!(pixel_jitter(0), [0.0, 0.0]);
    assert_eq!(lens_point(0, 16), [0.0, 0.0]);
    let mut seen = Vec::new();
    for i in 1..64 {
        let j = pixel_jitter(i);
        assert!(j.iter().all(|v| (-0.5..0.5).contains(v)), "{j:?}");
        assert!(!seen.contains(&j));
        seen.push(j);
        let l = lens_point(i, 64);
        assert!(l[0] * l[0] + l[1] * l[1] <= 1.0 + 1e-6);
    }
    // the jitter is spread over the pixel: the mean is near its centre
    let mean = seen.iter().fold([0.0f32; 2], |a, j| [a[0] + j[0], a[1] + j[1]]).map(|v| v / seen.len() as f32);
    assert!(mean[0].abs() < 0.05 && mean[1].abs() < 0.05, "{mean:?}");
}
