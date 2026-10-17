use super::*;

fn close(a: &[f32], b: &[f32], eps: f32) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
}

fn persp() -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.1, far: 100.0 };
    Camera::looking_at([1.0, 2.0, 5.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], &lens, [800.0, 600.0])
}

fn ortho() -> Camera {
    let lens = Lens::Orthographic { height: 4.0, near: 0.1, far: 50.0 };
    Camera::looking_at([0.0, 0.0, 10.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], &lens, [400.0, 200.0])
}

#[test]
fn inverse_undoes_the_matrix() {
    let c = persp();
    for m in [c.view, c.projection, c.view_projection()] {
        let inv = inverse(&m).unwrap();
        assert!(close(&multiply(&m, &inv), &IDENTITY, 1e-4), "{:?}", multiply(&m, &inv));
    }
    assert!(inverse(&[0.0; 16]).is_none());
}

#[test]
fn focal_length_and_field_of_view_convert_through_the_sensor() {
    // 24 mm sensor height: a 24 mm lens sees 2·atan(0.5) vertically
    assert!((fov_y(24.0) - 2.0 * 0.5f32.atan()).abs() < 1e-6);
    for mm in [12.0, 35.0, 50.0, 200.0] {
        assert!((focal_length(fov_y(mm)) - mm).abs() < 1e-3 * mm);
    }
    let lens = Lens::with_focal_length(50.0, 0.1, 10.0);
    let Lens::Perspective { fov_y: f, .. } = lens else { panic!() };
    assert!((focal_length(f) - 50.0).abs() < 1e-3);
}

#[test]
fn lenses_read_back_from_their_matrices() {
    for lens in [Lens::Perspective { fov_y: 0.7, near: 0.03, far: 60.0 }, Lens::Orthographic { height: 3.0, near: -2.0, far: 9.0 }] {
        let m = lens.matrix(1.5);
        let back = Lens::of_matrix(&m).unwrap();
        match (lens, back) {
            (Lens::Perspective { fov_y: a, near: n, far: f }, Lens::Perspective { fov_y: b, near: n2, far: f2 }) => {
                assert!((a - b).abs() < 1e-5 && (n - n2).abs() < 1e-4 && (f - f2).abs() < 0.05, "{back:?}");
            }
            (Lens::Orthographic { height: a, near: n, far: f }, Lens::Orthographic { height: b, near: n2, far: f2 }) => {
                assert!((a - b).abs() < 1e-5 && (n - n2).abs() < 1e-4 && (f - f2).abs() < 1e-4, "{back:?}");
            }
            _ => panic!("kind changed: {back:?}"),
        }
        assert!((Lens::aspect_of(&m).unwrap() - 1.5).abs() < 1e-6);
    }
    assert!(Lens::of_matrix(&IDENTITY).is_none());
}

#[test]
fn the_target_lands_in_the_centre_and_eye_and_forward_are_recovered() {
    let c = persp();
    let p = c.project([0.0, 0.0, 0.0]).unwrap();
    assert!(close(&p.screen, &[400.0, 300.0], 1e-3), "{p:?}");
    let d = (1.0f32 + 4.0 + 25.0).sqrt();
    assert!((p.view_depth - d).abs() < 1e-4);
    assert!(p.depth > 0.0 && p.depth < 1.0);
    assert!((c.view_depth_of(p.depth).unwrap() - d).abs() < 1e-3);
    assert!(close(&c.eye().unwrap(), &[1.0, 2.0, 5.0], 1e-4));
    let f = c.forward();
    assert!(close(&f, &crate::v3::norm([-1.0, -2.0, -5.0]), 1e-5));
    // behind the camera
    assert!(c.project([2.0, 4.0, 10.0]).is_none());
    // up on screen is up in the world
    let up = c.project([0.0, 0.5, 0.0]).unwrap();
    assert!(up.screen[1] < 300.0);
}

#[test]
fn rays_pass_through_the_points_they_were_cast_at() {
    for c in [persp(), ortho()] {
        for world in [[0.3, -0.2, 0.1], [-1.0, 0.5, -0.5], [0.0, 0.0, 0.0]] {
            let p = c.project(world).unwrap();
            let ray = c.ray(p.screen).unwrap();
            // the point lies on the ray
            let t = crate::v3::dot(crate::v3::sub(world, ray.origin), ray.dir);
            assert!(close(&ray.at(t), &world, 1e-3), "{:?} vs {world:?}", ray.at(t));
            assert!(t > 0.0);
        }
    }
    // orthographic rays are parallel to the viewing direction
    let c = ortho();
    let (a, b) = (c.ray([0.0, 0.0]).unwrap(), c.ray([400.0, 200.0]).unwrap());
    assert!(close(&a.dir, &b.dir, 1e-6) && close(&a.dir, &[0.0, 0.0, -1.0], 1e-6));
    assert!(c.is_orthographic() && !persp().is_orthographic());
    // a 4-unit-high view on 200 units: 50 units per world unit at any depth
    assert!((c.screen_scale(3.0) - 50.0).abs() < 1e-3);
}

#[test]
fn screen_and_device_coordinates_convert_both_ways() {
    let c = persp();
    assert!(close(&c.ndc_to_screen([-1.0, 1.0]), &[0.0, 0.0], 0.0));
    assert!(close(&c.ndc_to_screen([1.0, -1.0]), &[800.0, 600.0], 0.0));
    let s = [123.5, 456.25];
    assert!(close(&c.ndc_to_screen(c.screen_to_ndc(s)), &s, 1e-3));
    // points to pixels: the same camera, scaled positions
    let px = c.with_viewport([1600.0, 1200.0]);
    let (a, b) = (c.project([0.2, 0.1, 0.0]).unwrap(), px.project([0.2, 0.1, 0.0]).unwrap());
    assert!(close(&[a.screen[0] * 2.0, a.screen[1] * 2.0], &b.screen, 1e-3));
}

#[test]
fn facing_is_the_cosine_towards_the_camera() {
    let c = ortho();
    assert!((c.facing([0.0; 3], [0.0, 0.0, 1.0]) - 1.0).abs() < 1e-6);
    assert!((c.facing([0.0; 3], [0.0, 0.0, -1.0]) + 1.0).abs() < 1e-6);
    let c = persp();
    assert!(c.facing([0.0; 3], crate::v3::norm([1.0, 2.0, 5.0])) > 0.999);
}
