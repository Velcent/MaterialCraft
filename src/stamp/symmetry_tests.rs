//! Gate of roadmap step 30 for the stamp stage: on a symmetric test mesh (a quad whose UV layout is symmetric
//! about x = 0 and y = 0), a mirrored stroke painted with the mirror gives the same texels as the stroke itself,
//! and radial copies do not depend on which of the n rotations the stroke was painted at.

use super::tests::{camera, disc, image, mesh_of, red, surface};
use super::*;
use crate::mesh::test_util::quad;
use crate::paint::TiledImage;
use crate::paint::brush::Placement;
use crate::projection::{ViewProjector, ViewScene};
use crate::symmetry::Flip;

const SIZE: f32 = 256.0;

/// Paints `dabs` (centre, radius) as one stroke with `spec`, in texture space or through the straight-on view.
fn paint(spec: SymmetrySpec, dabs: &[([f32; 2], f32)], view: bool) -> TiledImage {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let projector = scene.projector(&mesh, camera([0.0, 0.0, 1.0], [0.0; 3])).unwrap();
    let space = if view {
        StrokeSpace::View(ViewSpace { projector: &projector, depth: None, placement: Placement::FacingView, max_angle: 85.0 })
    } else {
        StrokeSpace::Texture
    };
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space, factors: &[] };
    let mut stroke = StampStroke::new(spec, &[red(&mut img)]).unwrap();
    for (i, (c, r)) in dabs.iter().enumerate() {
        stroke.apply(&ctx, &disc(*c, *r, i as u64, 0.45, 0.9), &mut [red(&mut img)]).unwrap();
    }
    img
}

/// The screen position of the quad's centre in the straight-on view.
fn screen_centre() -> [f32; 2] {
    let mesh = mesh_of(vec![quad()]);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let projector = scene.projector(&mesh, camera([0.0, 0.0, 1.0], [0.0; 3])).unwrap();
    projector.to_screen([0.0; 3]).unwrap().pos
}

fn alpha(img: &TiledImage) -> Vec<f32> {
    let n = SIZE as u32;
    (0..n * n).map(|i| img.get(i % n, i / n)[3]).collect()
}

/// Largest difference, and the number of texels painted in one image but not the other (above `eps`).
fn compare(a: &TiledImage, b: &TiledImage, eps: f32) -> (f32, usize) {
    let (a, b) = (alpha(a), alpha(b));
    let diff = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).fold(0.0, f32::max);
    let odd = a.iter().zip(&b).filter(|(x, y)| (**x > eps) != (**y > eps)).count();
    (diff, odd)
}

/// A wavy stroke away from the symmetry plane and axes.
fn stroke_points(centre: [f32; 2], scale: f32) -> Vec<([f32; 2], f32)> {
    (0..14).map(|i| ([centre[0] + (20.0 + i as f32 * 4.25) * scale, centre[1] - (30.0 + (i as f32 * 0.6).sin() * 18.0) * scale], 7.5 * scale)).collect()
}

#[test]
fn a_mirrored_texture_stroke_with_the_mirror_equals_the_stroke_texel_for_texel() {
    let spec = SymmetrySpec::Mirror { axis: 0, offset: 0.0 };
    let s = stroke_points([128.0, 128.0], 1.0);
    let mirrored: Vec<_> = s.iter().map(|(c, r)| ([SIZE - c[0], c[1]], *r)).collect();
    let a = paint(spec, &s, false);
    let b = paint(spec, &mirrored, false);
    assert!(alpha(&a).iter().filter(|v| **v > 0.0).count() > 1000);
    assert_eq!(a.to_raw(), b.to_raw(), "texel for texel");
    // the copy lands on the mirrored texels: the image is symmetric itself
    let n = SIZE as u32;
    for y in (0..n).step_by(5) {
        for x in 0..n {
            assert_eq!(a.get(x, y), a.get(n - 1 - x, y), "({x}, {y})");
        }
    }
    // without the mirror the two strokes paint different texels
    assert_ne!(paint(SymmetrySpec::None, &s, false).to_raw(), paint(SymmetrySpec::None, &mirrored, false).to_raw());
}

#[test]
fn a_mirrored_view_stroke_with_the_mirror_equals_the_stroke() {
    let spec = SymmetrySpec::Mirror { axis: 0, offset: 0.0 };
    let c = screen_centre();
    let s = stroke_points(c, 1.0);
    let mirrored: Vec<_> = s.iter().map(|(p, r)| ([2.0 * c[0] - p[0], p[1]], *r)).collect();
    let a = paint(spec, &s, true);
    let b = paint(spec, &mirrored, true);
    let (diff, odd) = compare(&a, &b, 1e-3);
    assert!(alpha(&a).iter().filter(|v| **v > 0.5).count() > 1000);
    // the view's projection rounds differently on either side of the plane: equal to within 1e-4 (measured 1.3e-5)
    assert!(diff < 1e-4 && odd == 0, "max difference {diff}, {odd} texels painted on one side only");
}

#[test]
fn radial_copies_do_not_depend_on_which_rotation_was_painted() {
    let spec = SymmetrySpec::Radial { axis: 2, point: [0.0; 3], count: 4, flip: Flip::None };
    // texture space: the quarter turn about the texture's centre maps texels onto texels exactly
    let s = stroke_points([128.0, 128.0], 1.0);
    let turned: Vec<_> = s.iter().map(|(p, r)| ([128.0 - (p[1] - 128.0), 128.0 + (p[0] - 128.0)], *r)).collect();
    let a = paint(spec, &s, false);
    assert_eq!(a.to_raw(), paint(spec, &turned, false).to_raw());
    // four copies: the image turns into itself
    let n = SIZE as u32;
    for y in (0..n).step_by(3) {
        for x in 0..n {
            assert_eq!(a.get(x, y), a.get(n - 1 - y, x), "({x}, {y})");
        }
    }
    // through the view
    let c = screen_centre();
    let s = stroke_points(c, 0.8);
    let turned: Vec<_> = s.iter().map(|(p, r)| ([c[0] - (p[1] - c[1]), c[1] + (p[0] - c[0])], *r)).collect();
    let (diff, odd) = compare(&paint(spec, &s, true), &paint(spec, &turned, true), 1e-3);
    assert!(diff < 1e-4 && odd == 0, "max difference {diff}, {odd} texels painted on one side only");
}

#[test]
fn flipped_radial_copies_mirror_the_stamp_in_its_frame() {
    // a half disc (right half only) at the right; its half-turn copy lies at the left
    let half = |flip: Flip| {
        let mesh = mesh_of(vec![quad()]);
        let (map, index) = surface(&mesh);
        let mut img = image();
        let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
        let spec = SymmetrySpec::Radial { axis: 2, point: [0.0; 3], count: 2, flip };
        let mut stroke = StampStroke::new(spec, &[red(&mut img)]).unwrap();
        let mut d = disc([192.0, 128.0], 12.0, 0, 1.0, 1.0);
        let fp = &mut d.footprint;
        for y in 0..fp.height {
            for x in 0..fp.width {
                if (fp.x0 + x as i32) < 192 {
                    fp.values[y * fp.width + x] = 0.0;
                }
            }
        }
        stroke.apply(&ctx, &d, &mut [red(&mut img)]).unwrap();
        img
    };
    let plain = half(Flip::None);
    let flipped = half(Flip::U);
    // the original: right of its centre in both
    assert_eq!((plain.get(196, 128)[3], flipped.get(196, 128)[3]), (1.0, 1.0));
    // the copy is turned half way: its painted half lies left of x = 64 without the flip, right of it with it
    assert_eq!((plain.get(59, 127)[3], plain.get(68, 127)[3]), (1.0, 0.0));
    assert_eq!((flipped.get(59, 127)[3], flipped.get(68, 127)[3]), (0.0, 1.0));
}
