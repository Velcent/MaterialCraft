//! The step-37 gates in the core: a stencil scales each texel by the stencil image under its screen position, and
//! projection painting covers exactly the visible texels under the image, with the image's values.

use super::tests::{camera, disc, image, mesh_of, red, surface};
use super::*;
use crate::mesh::test_util::quad;
use crate::paint::brush::Placement;
use crate::paint::{Channel, PixelFormat, TiledImage};
use crate::projection::{Filter, ImageLevels, LevelChain, Repeat, ScreenImageTransform, ViewProjector, ViewScene};
use crate::tex::Tex;

const SIZE: u32 = 256;

/// A stencil of 0.25 / 0.75 squares, 8 texels wide.
fn checker(side: u32) -> TiledImage {
    let mut img = TiledImage::new(side, side, PixelFormat::from_id("rgba32f").unwrap()).unwrap();
    for y in 0..side {
        for x in 0..side {
            let v = if (x / 8 + y / 8) % 2 == 0 { 0.25 } else { 0.75 };
            img.set(x, y, [v, v, v, 1.0]);
        }
    }
    img
}

/// The stencil's value at a screen position, computed here from the placement (nearest texel, repeating).
fn stencil_at(img: &TiledImage, t: &ScreenImageTransform, screen: [f32; 2]) -> f32 {
    let uv = t.to_image(screen);
    let (w, h) = (img.width(), img.height());
    let x = ((uv[0] - uv[0].floor()) * w as f32) as u32;
    let y = ((uv[1] - uv[1].floor()) * h as f32) as u32;
    img.get(x.min(w - 1), y.min(h - 1))[0]
}

#[test]
fn the_stencil_factor_is_the_image_under_each_texels_screen_position() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let view = scene.projector(&mesh, camera([0.0, 0.0, 1.0], [0.0; 3])).unwrap();
    let stencil_img = checker(64);
    let mut placement = ScreenImageTransform::fitted(1.0, [400.0, 300.0]);
    placement.scale_about(0.4, [200.0, 150.0]);
    placement.rotate_about(30.0, [180.0, 140.0]);
    placement.repeat = Repeat::Both;
    let stencil = Stencil { image: &stencil_img, transform: placement };
    let centre = view.to_screen([0.0; 3]).unwrap().pos;
    let space = StrokeSpace::View(ViewSpace { projector: &view, depth: None, placement: Placement::FacingView, max_angle: 85.0 });
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space, factors: &[&stencil] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[red(&mut img)]).unwrap();
    // one hard dab over the whole view, full flow and opacity: the coverage is the factor itself
    stroke.apply(&ctx, &disc(centre, 400.0, 0, 1.0, 1.0), &mut [red(&mut img)]).unwrap();
    let (mut seen, mut low, mut high) = (0, 0, 0);
    let w = map.width();
    for y in 0..SIZE as usize {
        for x in 0..SIZE as usize {
            let Some((pos, _, _)) = map.point_index(y * w + x) else { continue };
            let screen = view.to_screen(pos).unwrap().pos;
            let want = stencil_at(&stencil_img, &placement, screen);
            let got = stroke.coverage(x, y);
            assert!((got - want).abs() < 1e-6, "texel ({x}, {y}) at {screen:?}: {got} vs {want}");
            assert!((img.get(x as u32, y as u32)[3] - want).abs() < 1e-6);
            seen += 1;
            low += usize::from(want == 0.25);
            high += usize::from(want == 0.75);
        }
    }
    assert!(seen > 60_000 && low > 10_000 && high > 10_000, "{seen} {low} {high}");
}

#[test]
fn projection_paint_covers_exactly_the_visible_texels_under_the_image() {
    // two quads facing the camera, the back one hidden behind the front one (UV: left half front, right half back)
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
    // an image of distinct values, placed over the middle of the front quad and turned
    let tex = Tex { w: 16, h: 12, px: (0..16 * 12).map(|i| [(i % 16) as f32 / 16.0, (i / 16) as f32 / 12.0, 0.5, 1.0]).collect() };
    let chain = LevelChain::default();
    let centre = view.to_screen([0.0; 3]).unwrap().pos;
    let placement = ScreenImageTransform { centre, size: [90.0, 70.0], rotation: 20.0, repeat: Repeat::None };
    let projected = ProjectedPaint::new(&map, &view, placement, Filter::Nearest).with_channel(Channel::BaseColor, ImageLevels::new(&tex, &chain));
    assert_eq!(projected.channels().collect::<Vec<_>>(), vec![Channel::BaseColor]);
    let source = projected.channel(Channel::BaseColor).unwrap();
    let space = StrokeSpace::View(ViewSpace { projector: &view, depth: Some(&depth), placement: Placement::FacingView, max_angle: 89.0 });
    let mut img = image();
    let ctx = StampContext { surface: &map, index: &index, space, factors: &[&projected] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[sourced(&mut img, source, StampMode::Paint)]).unwrap();
    stroke.apply(&ctx, &disc(centre, 400.0, 0, 1.0, 1.0), &mut [sourced(&mut img, source, StampMode::Paint)]).unwrap();

    let w = map.width();
    let (mut painted, mut hidden_under_image) = (0, 0);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let p = img.get(x, y);
            let Some((pos, normal, _)) = map.point_index(y as usize * w + x as usize) else {
                assert_eq!(p, [0.0; 4], "({x}, {y}) has no surface");
                continue;
            };
            let s = view.to_screen(pos).unwrap().pos;
            let uv = placement.to_image(s);
            let under = placement.covers(uv);
            let visible = view.visibility(pos, normal) > 0.0;
            if under && visible {
                painted += 1;
                // nearest texel of the image at the texel's screen position, laid on fully
                let (ix, iy) = (((uv[0] * 16.0) as usize).min(15), ((uv[1] * 12.0) as usize).min(11));
                let want = tex.px[iy * 16 + ix];
                assert_eq!(p, [want[0], want[1], want[2], 1.0], "({x}, {y})");
            } else {
                hidden_under_image += usize::from(under);
                assert_eq!(p, [0.0; 4], "({x}, {y}) under the image {under}, visible {visible}");
            }
        }
    }
    assert!(painted > 1000, "{painted}");
    assert!(hidden_under_image > 1000, "the hidden quad lies under the image too: {hidden_under_image}");
}

#[test]
fn erasing_ignores_a_projected_source_and_texels_without_a_value_keep_their_pixels() {
    struct Half;
    impl TexelValue for Half {
        fn value(&self, x: u32, _y: u32) -> Option<[f32; 4]> {
            (x < 128).then_some([0.0, 1.0, 0.0, 1.0])
        }
    }
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
    let mut img = image();
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[sourced(&mut img, &Half, StampMode::Paint)]).unwrap();
    stroke.apply(&ctx, &disc([128.0, 128.0], 20.0, 0, 1.0, 1.0), &mut [sourced(&mut img, &Half, StampMode::Paint)]).unwrap();
    assert_eq!(img.get(120, 128), [0.0, 1.0, 0.0, 1.0]);
    assert_eq!(img.get(136, 128), [0.0; 4]);
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[sourced(&mut img, &Half, StampMode::Erase)]).unwrap();
    stroke.apply(&ctx, &disc([128.0, 128.0], 20.0, 0, 1.0, 1.0), &mut [sourced(&mut img, &Half, StampMode::Erase)]).unwrap();
    assert_eq!(img.get(120, 128)[3], 0.0);
}

#[test]
fn the_projection_factor_keeps_paint_inside_the_image_and_off_2d_strokes() {
    let mesh = mesh_of(vec![quad()]);
    let (map, _) = surface(&mesh);
    let scene = ViewScene::new(&mesh, mesh.fit());
    let view = scene.projector(&mesh, camera([0.0, 0.0, 1.0], [0.0; 3])).unwrap();
    let placement = ScreenImageTransform { centre: [100.0, 100.0], size: [20.0, 20.0], rotation: 0.0, repeat: Repeat::None };
    let p = ProjectedPaint::new(&map, &view, placement, Filter::Bilinear);
    assert_eq!(p.factor(0, 0, Some([105.0, 95.0])), 1.0);
    assert_eq!(p.factor(0, 0, Some([115.0, 95.0])), 0.0);
    assert_eq!(p.factor(0, 0, None), 0.0);
    assert!(p.channel(Channel::BaseColor).is_none() && p.mask().is_none());
    assert_eq!(p.placement(), placement);
}

fn sourced<'a>(img: &'a mut TiledImage, source: &'a dyn TexelValue, mode: StampMode) -> ChannelTarget<'a> {
    ChannelTarget { source: Some(source), mode, ..red(img) }
}
