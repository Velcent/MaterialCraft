//! Clone and local-kernel strokes through the stamp stage.

use super::tests::{disc, image, mesh_of, surface};
use super::*;
use crate::mesh::test_util::quad;
use crate::paint::TiledImage;

/// A pattern no two texels share near the test area.
fn pattern(img: &mut TiledImage) {
    let all = img.all_tiles();
    img.update_tiles(&all, |r, px| {
        for (i, p) in px.iter_mut().enumerate() {
            let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
            *p = [(x % 17) as f32 / 16.0, (y % 13) as f32 / 12.0, ((x + y) % 7) as f32 / 6.0, 1.0];
        }
    });
}

#[test]
fn clone_copies_texel_for_texel_under_full_coverage() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let mut img = image();
    pattern(&mut img);
    let source = img.clone();
    fn target(img: &mut TiledImage) -> ChannelTarget<'_> {
        ChannelTarget { image: img, value: [0.0; 4], mode: StampMode::Clone { offset: [40, -12] }, srgb: false, source: None }
    }
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[target(&mut img)]).unwrap();
    for (i, x) in [100.0, 104.0, 108.0].into_iter().enumerate() {
        stroke.apply(&ctx, &disc([x, 100.0], 10.0, i as u64, 1.0, 1.0), &mut [target(&mut img)]).unwrap();
    }
    let mut copied = 0;
    for y in 85..115u32 {
        for x in 85..125u32 {
            let c = stroke.coverage(x as usize, y as usize);
            if c >= 1.0 {
                assert_eq!(img.get(x, y), source.get(x + 40, y - 12), "({x}, {y})");
                copied += 1;
            } else if c <= 0.0 {
                assert_eq!(img.get(x, y), source.get(x, y), "({x}, {y}) is outside the stroke");
            }
        }
    }
    assert!(copied > 300, "{copied}");
}

#[test]
fn local_strokes_need_their_kernels_and_blur_softens_an_edge() {
    let mesh = mesh_of(vec![quad()]);
    let (map, index) = surface(&mesh);
    let mut img = image();
    let all = img.all_tiles();
    img.update_tiles(&all, |r, px| {
        for (i, p) in px.iter_mut().enumerate() {
            *p = if r.x as usize + i % r.w as usize >= 100 { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] };
        }
    });
    let before = img.clone();
    fn target(img: &mut TiledImage) -> ChannelTarget<'_> {
        ChannelTarget { image: img, value: [0.0; 4], mode: StampMode::Local, srgb: false, source: None }
    }
    let ctx = StampContext { surface: &map, index: &index, space: StrokeSpace::Texture, factors: &[] };
    let mut stroke = StampStroke::new(SymmetrySpec::None, &[target(&mut img)]).unwrap();
    let dab = |i: u64| disc([100.0, 60.0 + i as f32 * 4.0], 12.0, i, 1.0, 1.0);
    assert_eq!(stroke.apply(&ctx, &dab(0), &mut [target(&mut img)]), Err(StampError::NoKernel));
    stroke.set_kernels(vec![Some(Box::new(BlurKernel { strength: 1.0 }))]);
    for i in 0..10 {
        stroke.apply(&ctx, &dab(i), &mut [target(&mut img)]).unwrap();
    }
    let (left, right) = (img.get(98, 70)[0], img.get(101, 70)[0]);
    assert!(left > 0.05 && right < 0.95, "the edge is soft: {left} {right}");
    assert_eq!(img.get(98, 20), before.get(98, 20), "off the stroke nothing changes");
    assert_eq!(img.get(60, 70), before.get(60, 70), "flat areas stay flat");
    assert!(!stroke.touched().is_empty());
}
