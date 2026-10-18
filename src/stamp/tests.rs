use super::*;
use crate::camera::{Camera, Lens};
use crate::mesh::test_util::{quad, seamed_cube};
use crate::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use crate::paint::brush::Placement;
use crate::paint::{PixelFormat, TiledImage};
use crate::projection::{ViewProjector, ViewScene};

const SIZE: u32 = 256;

pub(super) fn mesh_of(parts: Vec<MeshPart>) -> Mesh {
    Mesh { materials: vec!["m".into()], parts }
}

pub(super) fn surface(mesh: &Mesh) -> (SurfaceMap, TexelIndex) {
    let map = SurfaceMap::build(mesh, SurfaceKey::square(mesh, 0, SIZE)).unwrap();
    let index = TexelIndex::build(&map);
    (map, index)
}

pub(super) fn image() -> TiledImage {
    TiledImage::new(SIZE, SIZE, PixelFormat::from_id("rgba32f").unwrap()).unwrap()
}

/// A hard round dab of `radius` at `center`, coverage 1 inside.
pub(super) fn disc(center: [f32; 2], radius: f32, index: u64, flow: f32, opacity: f32) -> DabSample {
    let r = radius.ceil() as i32 + 1;
    let (x0, y0) = (center[0].floor() as i32 - r, center[1].floor() as i32 - r);
    let n = (2 * r + 1) as usize;
    let mut values = vec![0.0; n * n];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x0 as f32 + x as f32 + 0.5 - center[0], y0 as f32 + y as f32 + 0.5 - center[1]);
            values[y * n + x] = if (dx * dx + dy * dy).sqrt() <= radius { flow } else { 0.0 };
        }
    }
    DabSample { center, radius, opacity, index, footprint: Footprint::new(x0, y0, n, n, values).unwrap() }
}

fn erase(img: &mut TiledImage) -> ChannelTarget<'_> {
    ChannelTarget { image: img, value: [0.0; 4], mode: StampMode::Erase, srgb: false, source: None }
}

pub(super) fn red(img: &mut TiledImage) -> ChannelTarget<'_> {
    ChannelTarget { image: img, value: [1.0, 0.0, 0.0, 1.0], mode: StampMode::Paint, srgb: false, source: None }
}

#[test]
fn footprints_sample_bilinearly_and_are_zero_outside() {
    let f = Footprint::new(10, 20, 2, 1, vec![1.0, 0.0]).unwrap();
    assert_eq!(f.sample([10.5, 20.5]), 1.0);
    assert_eq!(f.sample([11.0, 20.5]), 0.5);
    assert_eq!(f.sample([5.0, 5.0]), 0.0);
    assert!(Footprint::new(0, 0, 2, 2, vec![0.0; 3]).is_none());
    // a coarser grid: the same values over twice the frame area
    let coarse = Footprint::new(5, 10, 2, 1, vec![1.0, 0.0]).unwrap().with_scale(2.0);
    assert_eq!(coarse.bounds(), [10.0, 20.0, 14.0, 22.0]);
    assert!((coarse.sample([11.0, 21.0]) - 1.0).abs() < 1e-6 && coarse.sample([13.0, 21.0]) < 1e-6);
    assert!((coarse.sample([12.0, 21.0]) - 0.5).abs() < 1e-6);
}

#[test]
fn texture_strokes_build_up_flow_under_the_opacity_ceiling() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
    let mut last = 0.0;
    for i in 0..40 {
        stroke.apply(&ctx, &disc([100.0, 100.0], 10.0, i, 0.3, 0.6), &mut [red(&mut img)]).unwrap();
        let c = stroke.coverage(100, 100);
        assert!(c >= last && c <= 0.6, "dab {i}: {c}");
        last = c;
        if i == 0 {
            assert!((c - 0.18).abs() < 1e-6, "one dab adds flow × ceiling");
        }
    }
    let p = img.get(100, 100);
    assert!((p[3] - 0.6).abs() < 1e-3 && p[0] == 1.0, "{p:?}");
    assert_eq!(img.get(120, 100), [0.0; 4], "outside the dab nothing changes");
    assert_eq!(stroke.touched(), vec![(0, 0)]);
    // the eraser takes coverage away again
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
    let mut e = StampStroke::new(SymmetrySpec::None, &[erase(&mut img)]).unwrap();
    e.apply(&ctx, &disc([100.0, 100.0], 10.0, 0, 1.0, 1.0), &mut [erase(&mut img)]).unwrap();
    assert_eq!(img.get(100, 100)[3], 0.0);
}

#[test]
fn quick_masks_hold_paint_back() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let mut mask = TiledImage::new(SIZE, SIZE, PixelFormat::from_id("gray8").unwrap()).unwrap();
    for y in 0..SIZE {
        for x in 0..SIZE / 2 {
            mask.set(x, y, [1.0; 4]);
        }
    }
    let quick = QuickMask { image: &mask, inverted: false };
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[&quick] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
    stroke.apply(&ctx, &disc([128.0, 60.0], 20.0, 0, 1.0, 1.0), &mut [red(&mut img)]).unwrap();
    assert_eq!(img.get(120, 60)[3], 1.0);
    assert_eq!(img.get(136, 60)[3], 0.0);
}

#[test]
fn the_same_dabs_give_the_same_bits() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let run = || {
        let mut img = image();
        let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
        let mut s = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
        for i in 0..30 {
            let c = [20.0 + i as f32 * 7.3, 30.0 + (i as f32 * 0.7).sin() * 40.0];
            s.apply(&ctx, &disc(c, 17.5, i, 0.21, 0.9), &mut [red(&mut img)]).unwrap();
        }
        img.to_raw()
    };
    assert_eq!(run(), run());
}

/// A camera looking at `target` from `dir` (unit sphere of the fitted model at the origin).
pub(super) fn camera(dir: [f32; 3], target: [f32; 3]) -> Camera {
    let lens = Lens::Perspective { fov_y: 40f32.to_radians(), near: 0.03, far: 60.0 };
    let d = crate::v3::norm(dir);
    let eye = crate::v3::add(target, crate::v3::mul(d, 3.0));
    Camera::looking_at(eye, target, [0.0, 1.0, 0.0], &lens, [400.0, 300.0])
}

#[test]
fn a_stamp_across_a_uv_seam_paints_both_sides_alike() {
    let mesh = mesh_of(vec![seamed_cube()]);
    let (map, index) = surface(&mesh);
    let fit = mesh.fit();
    let scene = ViewScene::new(&mesh, fit);
    // the seam is the cube's vertical edge at x = -1, z = 1 (u = 0 on one face, u = 1 on the other)
    let edge_world = fit.apply([-1.0, 0.0, 1.0]);
    let cam = camera([-1.0, 0.2, 1.0], edge_world);
    let view = scene.projector(&mesh, cam).unwrap();
    let depth = DepthMap::build(&mesh, &view, &|_| true);
    let centre = view.to_screen([-1.0, 0.0, 1.0]).unwrap().pos;
    for placement in [Placement::FacingView, Placement::Wrapped] {
        let space = StrokeSpace::View(ViewSpace { projector: &view, depth: Some(&depth), placement, max_angle: 85.0 });
        let mut img = image();
        let ctx = StampContext { surface: &map, index: &index, space, factors: &[] };
        let mut stroke = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
        stroke.apply(&ctx, &disc(centre, 30.0, 0, 1.0, 1.0), &mut [red(&mut img)]).unwrap();
        // texels just inside each side of the seam, half-way up the strip (v = 0.5)
        let y = SIZE / 2;
        let left = img.get(1, y)[3];
        let right = img.get(SIZE - 2, y)[3];
        assert!(left > 0.99 && right > 0.99, "{placement:?}: {left} {right}");
        // the gutter beyond the strip's ends is painted as well
        assert!(img.get(0, y)[3] > 0.99 && img.get(SIZE - 1, y)[3] > 0.99, "{placement:?}");
        // the far faces (hidden from this camera) stay clean
        assert_eq!(img.get(SIZE / 2, y)[3], 0.0, "{placement:?}");
    }
}

#[test]
fn hidden_and_turned_away_surfaces_stay_clean() {
    // two quads facing the camera, the second behind the first and shifted in UV to the right half
    let mut front = quad();
    front.uv_sets[0].iter_mut().for_each(|uv| uv[0] *= 0.5);
    let mut back = quad();
    back.positions.iter_mut().for_each(|p| p[2] -= 1.0);
    back.uv_sets[0].iter_mut().for_each(|uv| uv[0] = 0.5 + uv[0] * 0.5);
    let mesh = mesh_of(vec![front, back]);
    let (map, index) = surface(&mesh);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let view = scene.projector(&mesh, camera([0.0, 0.0, 1.0], [0.0; 3])).unwrap();
    let depth = DepthMap::build(&mesh, &view, &|_| true);
    let centre = view.to_screen([0.0, 0.0, 0.0]).unwrap().pos;
    let paint = |depth: Option<&DepthMap>| {
        let space = StrokeSpace::View(ViewSpace { projector: &view, depth, placement: Placement::FacingView, max_angle: 85.0 });
        let mut img = image();
        let ctx = StampContext { surface: &map, index: &index, space, factors: &[] };
        let mut stroke = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
        stroke.apply(&ctx, &disc(centre, 20.0, 0, 1.0, 1.0), &mut [red(&mut img)]).unwrap();
        img
    };
    let with = paint(Some(&depth));
    let (front_mid, back_mid) = ((SIZE / 4, SIZE / 2), (3 * SIZE / 4, SIZE / 2));
    assert_eq!(with.get(front_mid.0, front_mid.1)[3], 1.0);
    assert_eq!(with.get(back_mid.0, back_mid.1)[3], 0.0, "occluded");
    let without = paint(None);
    assert_eq!(without.get(back_mid.0, back_mid.1)[3], 1.0, "without the depth test the hidden quad is painted");
    // a camera behind both quads sees their backs: nothing is painted
    let behind = scene.projector(&mesh, camera([0.0, 0.0, -1.0], [0.0; 3])).unwrap();
    let c = behind.to_screen([0.0, 0.0, 0.0]).unwrap().pos;
    let space = StrokeSpace::View(ViewSpace { projector: &behind, depth: None, placement: Placement::FacingView, max_angle: 85.0 });
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space, factors: &[] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
    assert!(stroke.apply(&ctx, &disc(c, 20.0, 0, 1.0, 1.0), &mut [red(&mut img)]).unwrap().is_empty());
}

#[test]
fn mirror_symmetry_paints_the_mirrored_place() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let view = scene.projector(&mesh, camera([0.0, 0.0, 1.0], [0.0; 3])).unwrap();
    let centre = view.to_screen([0.5, 0.0, 0.0]).unwrap().pos;
    let space = StrokeSpace::View(ViewSpace { projector: &view, depth: None, placement: Placement::Wrapped, max_angle: 85.0 });
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space, factors: &[] };
    let mut stroke = StampStroke::new(SymmetrySpec::Mirror { axis: 0, offset: 0.0 }, &[red(&mut img)]).unwrap();
    stroke.apply(&ctx, &disc(centre, 10.0, 0, 1.0, 1.0), &mut [red(&mut img)]).unwrap();
    // x = 0.5 is u = 0.75, its mirror x = -0.5 is u = 0.25
    let y = SIZE / 2;
    assert_eq!(img.get(3 * SIZE / 4, y)[3], 1.0);
    assert_eq!(img.get(SIZE / 4, y)[3], 1.0);
    assert_eq!(img.get(SIZE / 2, y)[3], 0.0);
}

#[test]
fn stamps_refuse_images_of_another_size() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let mut small = TiledImage::new(8, 8, PixelFormat::from_id("gray8").unwrap()).unwrap();
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
    let mut s = StampStroke::new(SymmetrySpec::None, &[red(&mut small)]).unwrap();
    assert_eq!(s.apply(&ctx, &disc([5.0, 5.0], 2.0, 0, 1.0, 1.0), &mut [red(&mut small)]), Err(StampError::SizeMismatch));
    let mut img = image();
    assert!(matches!(StampStroke::new(SymmetrySpec::None, &[red(&mut img), red(&mut small)]), Err(StampError::SizeMismatch)));
    let mut s = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
    assert_eq!(s.apply(&ctx, &disc([5.0, 5.0], 2.0, 0, 1.0, 1.0), &mut []), Err(StampError::TargetsChanged));
}

#[test]
fn view_projector_screen_scale_matches_the_camera() {
    let mesh = mesh_of(vec![quad()]);
    let fit = mesh.fit();
    let scene = ViewScene::new(&mesh, fit);
    let cam = camera([0.0, 0.0, 1.0], [0.0; 3]);
    let view = scene.projector(&mesh, cam).unwrap();
    let k = screen_scale_at(&view, [0.0; 3]).unwrap();
    let s = view.to_screen([0.0; 3]).unwrap();
    let expected = cam.screen_scale(s.view_depth) * fit.scale;
    assert!((k - expected).abs() / expected < 1e-2, "{k} vs {expected}");
}
