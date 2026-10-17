use super::*;
use crate::camera::{Camera, Lens};
use crate::mesh::test_util::{mirrored_quad, quad, seamed_cube};
use crate::mesh::{Fit, Mesh, MeshPart};
use crate::projection::{Projector, Repeat, SurfacePoint};

fn mesh_of(parts: Vec<MeshPart>) -> Mesh {
    Mesh { materials: vec!["m".into()], parts }
}

/// A real model: the viewport's built-in shapes (sphere, torus, mat ball, ...), tessellated as drawn.
fn shape(id: u32) -> Mesh {
    let m = crate::render3d::build_mesh(id, 48, 1.0, None, 0.0, 0.0);
    mesh_of(vec![MeshPart::from_viewport("shape", 0, &m)])
}

/// Orbit cameras around the fitted model (unit sphere at the origin), as the viewport places them.
fn cameras(viewport: [f32; 2]) -> Vec<Camera> {
    let persp = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    let ortho = Lens::Orthographic { height: 2.6, near: 0.03, far: 60.0 };
    let mut out = Vec::new();
    for (yaw, pitch) in [(0.0f32, 0.0f32), (0.7, 0.4), (2.5, -0.6), (-1.9, 1.2)] {
        let eye = [3.2 * yaw.sin() * pitch.cos(), 3.2 * pitch.sin(), 3.2 * yaw.cos() * pitch.cos()];
        out.push(Camera::looking_at(eye, [0.05, -0.03, 0.0], [0.0, 1.0, 0.0], &persp, viewport));
        out.push(Camera::looking_at(eye, [0.0; 3], [0.0, 1.0, 0.0], &ortho, viewport));
    }
    out
}

/// Picks every `step`-th pixel centre and projects the hit back: (hits, largest distance in pixels).
fn round_trip(mesh: &Mesh, camera: Camera, step: usize) -> (usize, f32) {
    let scene = ViewScene::new(mesh, mesh.fit());
    let view = scene.projector(mesh, camera).unwrap();
    let (w, h) = (camera.viewport[0] as usize, camera.viewport[1] as usize);
    let (mut hits, mut worst) = (0, 0.0f32);
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let s = [x as f32 + 0.5, y as f32 + 0.5];
            let Some(hit) = view.pick(s) else { continue };
            hits += 1;
            let back = view.to_screen(hit.position).unwrap();
            worst = worst.max(((back.pos[0] - s[0]).powi(2) + (back.pos[1] - s[1]).powi(2)).sqrt());
            assert!((hit.barycentric.iter().sum::<f32>() - 1.0).abs() < 1e-4);
            assert!((crate::v3::len(hit.normal) - 1.0).abs() < 1e-3);
            assert!(back.depth > 0.0 && back.depth < 1.0 && back.view_depth > 0.0);
        }
    }
    (hits, worst)
}

#[test]
fn pick_and_to_screen_round_trip_within_half_a_pixel() {
    let mut meshes = vec![mesh_of(vec![quad()]), mesh_of(vec![mirrored_quad()]), mesh_of(vec![seamed_cube()])];
    // two parts sharing one fit
    let mut moved = seamed_cube();
    moved.positions.iter_mut().for_each(|p| p[0] += 3.0);
    meshes.push(mesh_of(vec![seamed_cube(), moved]));
    for id in [0, 7, 8, 12] {
        meshes.push(shape(id));
    }
    let mut overall = 0.0f32;
    for (i, mesh) in meshes.iter().enumerate() {
        for camera in cameras([320.0, 240.0]) {
            let (hits, worst) = round_trip(mesh, camera, 3);
            assert!(hits > 50, "mesh {i}: only {hits} hits");
            assert!(worst < 0.5, "mesh {i}: round trip off by {worst} px");
            overall = overall.max(worst);
        }
    }
    eprintln!("pick → to_screen round trip: worst {overall} px");
}

#[test]
fn picks_report_part_triangle_uv_and_normal() {
    // the quad (2 × 2, facing +z) fills an orthographic view of height 2 straight on
    let mesh = mesh_of(vec![quad()]);
    let fit = Fit::IDENTITY;
    let scene = ViewScene::new(&mesh, fit);
    let lens = Lens::Orthographic { height: 2.0, near: 0.1, far: 10.0 };
    let cam = Camera::looking_at([0.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [100.0, 100.0]);
    let view = scene.projector(&mesh, cam).unwrap();
    let hit = view.pick([25.0, 75.0]).unwrap();
    assert_eq!(hit.part, 0);
    assert!(crate::mesh::test_util::close(&hit.position, &[-0.5, -0.5, 0.0], 1e-4), "{hit:?}");
    // u along +x, v along −y (origin top-left): screen and UV agree
    assert!(crate::mesh::test_util::close(&hit.uv, &[0.25, 0.75], 1e-4), "{hit:?}");
    assert!(crate::mesh::test_util::close(&hit.normal, &[0.0, 0.0, 1.0], 1e-5));
    assert!((hit.distance - 4.9).abs() < 1e-4, "from the near plane: {}", hit.distance);
    assert!(view.pick([-5.0, 50.0]).is_none(), "outside the quad");
    // a missing UV set reads as zero; another set can be chosen
    let mut two = quad();
    two.uv_sets.push(two.uv_sets[0].iter().map(|uv| [uv[0] * 0.5, uv[1] * 0.5]).collect());
    let mesh2 = mesh_of(vec![two]);
    let mut scene2 = ViewScene::new(&mesh2, fit);
    scene2.set_uv_set(0, 1);
    let hit = scene2.projector(&mesh2, cam).unwrap().pick([25.0, 75.0]).unwrap();
    assert!(crate::mesh::test_util::close(&hit.uv, &[0.125, 0.375], 1e-4));
    scene2.set_uv_set(0, 5);
    assert_eq!(scene2.projector(&mesh2, cam).unwrap().pick([25.0, 75.0]).unwrap().uv, [0.0, 0.0]);
    // hidden parts are not picked
    scene2.set_shown(0, false);
    assert!(scene2.projector(&mesh2, cam).unwrap().pick([25.0, 75.0]).is_none());
    // a scene does not pair with another mesh
    assert!(scene.projector(&mesh_of(vec![seamed_cube()]), cam).is_none());
}

#[test]
fn back_faces_are_occluded_and_face_away() {
    let mesh = mesh_of(vec![seamed_cube()]);
    let scene = ViewScene::new(&mesh, mesh.fit());
    for lens in [Lens::Perspective { fov_y: 0.7, near: 0.03, far: 60.0 }, Lens::Orthographic { height: 3.0, near: 0.03, far: 60.0 }] {
        let cam = Camera::looking_at([0.0, 0.0, 4.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [200.0, 200.0]);
        let view = scene.projector(&mesh, cam).unwrap();
        // front face z = 1, back face z = −1 (object space)
        let (front, back) = ([0.3, 0.2, 1.0], [0.3, 0.2, -1.0]);
        assert!(!view.occluded(front));
        assert!(view.occluded(back));
        assert!(view.facing(front, [0.0, 0.0, 1.0]) > 0.9);
        assert!(view.facing(back, [0.0, 0.0, -1.0]) < -0.9);
        assert!(view.visibility(front, [0.0, 0.0, 1.0]) > 0.9);
        assert_eq!(view.visibility(back, [0.0, 0.0, -1.0]), 0.0);
        // picks on the front face are visible
        for s in [[70.0, 75.0], [100.0, 100.0], [128.0, 90.0]] {
            let hit = view.pick(s).unwrap();
            assert!(!view.occluded(hit.position), "{hit:?}");
        }
    }
    // behind the camera: invisible
    let cam = Camera::looking_at([0.0, 0.0, 0.5], [0.0, 0.0, 2.0], [0.0, 1.0, 0.0], &Lens::Perspective { fov_y: 0.7, near: 0.03, far: 60.0 }, [10.0, 10.0]);
    let view = scene.projector(&mesh, cam).unwrap();
    assert!(view.to_screen([0.0, 0.0, -1.0]).is_none());
    assert_eq!(view.visibility([0.0, 0.0, -1.0], [0.0, 0.0, -1.0]), 0.0);
}

#[test]
fn screen_image_transform_maps_both_ways() {
    let mut t = ScreenImageTransform::fitted(2.0, [400.0, 300.0]);
    assert_eq!(t.size, [400.0, 200.0]);
    assert_eq!(t.to_image([200.0, 150.0]), [0.5, 0.5]);
    assert_eq!(t.to_image([0.0, 50.0]), [0.0, 0.0]);
    t.rotate_about(90.0, [200.0, 150.0]);
    // turned counter-clockwise: the image's right edge now points up on screen
    let right = t.to_screen([1.0, 0.5]);
    assert!((right[0] - 200.0).abs() < 1e-3 && (right[1] - (150.0 - 200.0)).abs() < 1e-3, "{right:?}");
    t.scale_about(0.5, [0.0, 0.0]);
    t.translate([10.0, -4.0]);
    for uv in [[0.1, 0.9], [0.5, 0.5], [1.7, -0.3]] {
        let back = t.to_image(t.to_screen(uv));
        assert!((back[0] - uv[0]).abs() < 1e-4 && (back[1] - uv[1]).abs() < 1e-4);
    }
    t.rotation = 47.0;
    assert_eq!(t.snapped_rotation(15.0), 45.0);
    assert_eq!(t.snapped_rotation(0.0), 47.0);
    assert!(!t.covers([1.2, 0.5]));
    t.repeat = Repeat::U;
    assert!(t.covers([1.2, 0.5]) && !t.covers([0.5, -0.1]));
    t.repeat = Repeat::Both;
    assert!(t.covers([-3.0, 7.0]));
}

#[test]
fn screen_projector_paints_visible_texels_through_the_image() {
    let mesh = mesh_of(vec![seamed_cube()]);
    let scene = ViewScene::new(&mesh, Fit::IDENTITY);
    let lens = Lens::Orthographic { height: 4.0, near: 0.1, far: 20.0 };
    let cam = Camera::looking_at([0.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, 0.0], &lens, [200.0, 200.0]);
    let view = scene.projector(&mesh, cam).unwrap();
    let proj = ScreenProjector { view: &view, image: ScreenImageTransform::fitted(1.0, [200.0, 200.0]) };
    let point = |pos, normal| SurfacePoint { uv: [0.0; 2], pos, normal, texel_len: 1.0 / 512.0, world_len: 0.0 };
    // the front face's centre is the image's centre, fully covered
    let t = proj.project(&point([0.0, 0.0, 1.0], [0.0, 0.0, 1.0]));
    assert!((t.used()[0].uv[0] - 0.5).abs() < 1e-4 && (t.used()[0].uv[1] - 0.5).abs() < 1e-4);
    assert!((t.coverage - 1.0).abs() < 1e-4);
    // the top-left corner of the front face (x −1, y +1) is a quarter in from the image's top-left
    let t = proj.project(&point([-1.0, 1.0, 1.0], [0.0, 0.0, 1.0]));
    assert!((t.used()[0].uv[0] - 0.25).abs() < 1e-4 && (t.used()[0].uv[1] - 0.25).abs() < 1e-4, "{t:?}");
    // the back face is hidden
    assert_eq!(proj.project(&point([0.0, 0.0, -1.0], [0.0, 0.0, -1.0])).coverage, 0.0);
    // a smaller, non-repeating image does not reach the corner
    let small = ScreenProjector { view: &view, image: ScreenImageTransform { size: [50.0, 50.0], ..proj.image } };
    assert_eq!(small.project(&point([-0.9, 0.9, 1.0], [0.0, 0.0, 1.0])).coverage, 0.0);
}

#[test]
fn a_depth_buffer_from_the_same_scene_agrees() {
    let mesh = shape(12);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let cam = cameras([96.0, 64.0])[2];
    let view = scene.projector(&mesh, cam).unwrap();
    // the buffer a depth-tested rasteriser would draw, made from picks
    let depth: Vec<f32> = (0..64 * 96)
        .map(|i| view.pick([(i % 96) as f32 + 0.5, (i / 96) as f32 + 0.5]).and_then(|h| view.to_screen(h.position)).map_or(1.0, |s| s.depth))
        .collect();
    let buffer = DepthBuffer { width: 96, height: 64, depth: &depth };
    let points: Vec<_> = mesh.parts[0]
        .triangles
        .iter()
        .map(|t| {
            let p = t.map(|i| mesh.parts[0].positions[i as usize]);
            [0, 1, 2].map(|k| (p[0][k] + p[1][k] + p[2][k]) / 3.0)
        })
        .collect();
    let r = compare_depth(&view, &buffer, points.iter().copied(), 1, 0.01);
    assert!(r.covered > 500 && r.pixel_percent() == 100.0, "{}", r.pixel_percent());
    assert!(r.worst_round_trip < 0.5);
    assert!(r.points > 100 && r.occluded_by_ray > 0, "{r:?}");
    assert!(r.point_percent() > 99.0, "{} {r:?}", r.point_percent());
    assert!(r.points_ambiguous < r.points);
    assert_eq!(r.map.len(), 96 * 64);
    // an empty buffer disagrees wherever the shape is
    let empty = vec![1.0; 96 * 64];
    let r = compare_depth(&view, &DepthBuffer { width: 96, height: 64, depth: &empty }, [], 2, 0.01);
    assert_eq!(r.pixel_percent(), 0.0);
    assert!(r.map.contains(&PixelAgreement::ProjectorOnly));
}
