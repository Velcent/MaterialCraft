//! Image fills through their projection, the [`FillContext`] hook and the warnings for fills that need a mesh.

use super::*;
use crate::paint::effect::EvalTarget;
use crate::paint::{ContentEffect, EffectKind, Fill, FillSource, ImageFill, PixelFormat};
use crate::projection::{Filter, ImagePlacement, Projection, ProjectionMode, Repeat, Tap};

const R: Channel = Channel::Roughness;

/// A grey image whose texel `x` has value `x / 10`.
fn ramp(w: u32, h: u32) -> TiledImage {
    let mut img = TiledImage::new(w, h, PixelFormat::from_id("gray16").unwrap()).unwrap();
    for y in 0..h {
        for x in 0..w {
            img.set(x, y, [x as f32 / 10.0; 4]);
        }
    }
    img
}

fn image_fill(img: TiledImage, projection: Projection) -> Fill {
    let mut f = ImageFill::default();
    f.set_image(R, img);
    f.set_value(Channel::BaseColor, [0.2, 0.4, 0.6, 1.0]);
    Fill::new(FillSource::Image(f)).with_projection(projection)
}

fn set_with(fill: Fill) -> TextureSet {
    let mut s = TextureSet::new("s", "S", (4, 4)).with_channel(R).with_channel(Channel::BaseColor);
    s.layers.push(Layer::new("base", "Base", LayerKind::Fill(Fill::uniform(vec![(R, [0.9; 4])]))).with_blend(R, BlendMode::Normal, 1.0));
    s.layers.push(Layer::new("img", "Image", LayerKind::Fill(fill)).with_blend(R, BlendMode::Normal, 1.0).with_blend(
        Channel::BaseColor,
        BlendMode::Normal,
        1.0,
    ));
    s
}

/// Row `y` of the roughness composite.
fn row_at(s: &TextureSet, c: Compositor, y: usize) -> Vec<f32> {
    let (tex, _) = c.composite_tex(R);
    (0..s.resolution.0 as usize).map(|x| tex.px[y * tex.w + x][0]).collect()
}

fn row(s: &TextureSet, c: Compositor) -> Vec<f32> {
    row_at(s, c, 0)
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 2e-3)
}

#[test]
fn texture_space_image_of_the_set_size_is_copied_exactly() {
    let s = set_with(image_fill(ramp(4, 4), Projection::default()));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.0, 0.1, 0.2, 0.3]));
    assert!(Compositor::new(&s).warnings(R).is_empty());
    // the constant channel ignores the projection
    let (tex, _) = Compositor::new(&s).composite_tex(Channel::BaseColor);
    assert!((tex.px[5][2] - 0.6).abs() < 1e-3);
}

#[test]
fn larger_images_are_prefiltered_and_smaller_ones_interpolated() {
    // 8 texels onto 4: each output texel averages two image texels
    let s = set_with(image_fill(ramp(8, 8), Projection::default()));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.05, 0.25, 0.45, 0.65]));
    let nearest = Projection { filter: Filter::Nearest, ..Projection::default() };
    let s = set_with(image_fill(ramp(8, 8), nearest));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.1, 0.3, 0.5, 0.7]));
    // 2 texels onto 4, repeating: bilinear between texel centres, wrapping at the edges
    let s = set_with(image_fill(ramp(2, 2), Projection::default()));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.025, 0.025, 0.075, 0.075]));
}

#[test]
fn uv_placement_tiles_and_no_repeat_leaves_the_outside_uncovered() {
    let tiled = Projection::uv(ImagePlacement { tiling: [2.0, 2.0], ..ImagePlacement::default() });
    let s = set_with(image_fill(ramp(2, 2), Projection { filter: Filter::Nearest, ..tiled.clone() }));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.1, 0.0, 0.1, 0.0]));
    // half size, centred, not repeating: the texels around it show the layer below
    let s = set_with(image_fill(ramp(4, 4), Projection { filter: Filter::Nearest, repeat: Repeat::None, ..tiled }));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.9; 4]));
    assert!(close(&row_at(&s, Compositor::new(&s), 1), &[0.9, 0.1, 0.3, 0.9]));
}

#[test]
fn volume_projections_are_reported_not_guessed() {
    let planar = Projection { mode: ProjectionMode::default_of("planar").unwrap(), ..Projection::default() };
    let s = set_with(image_fill(ramp(4, 4), planar.clone()));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.9; 4]));
    assert_eq!(Compositor::new(&s).warnings(R), vec![CompositeWarning::FillNeedsMesh { layer: "img".into(), effect: None, mode: "planar".into() }]);
    // a fill effect with such a projection is left out with the same warning
    let mut s = set_with(Fill::uniform(vec![(R, [0.3; 4])]));
    let fx = ContentEffect::new(Vec::new(), EffectKind::Fill(image_fill(ramp(4, 4), planar))).with_blend(R, BlendMode::Normal, 1.0);
    s.layers[1].effects.push(fx);
    assert!(close(&row(&s, Compositor::new(&s)), &[0.3; 4]));
    assert_eq!(Compositor::new(&s).warnings(R), vec![CompositeWarning::FillNeedsMesh { layer: "img".into(), effect: Some(0), mode: "planar".into() }]);
}

/// Evaluates material fills as the image coordinate's u (to see the projection reach the source).
struct UAsValue;

impl FillSources for UAsValue {
    fn handles(&self, source: &FillSource) -> bool {
        matches!(source, FillSource::Material { .. })
    }

    fn sample(&self, request: &FillRequest, taps: &[Tap], out: &mut [[f32; 4]]) -> bool {
        assert_eq!(request.target, EvalTarget::Channel(R));
        for (t, o) in taps.iter().zip(out) {
            *o = [t.uv[0], t.uv[0], t.uv[0], 1.0];
        }
        true
    }
}

#[test]
fn other_sources_are_evaluated_through_the_chained_context() {
    let material = Fill::new(FillSource::Material { path: "materials/rust.mca".into(), params: vec![] });
    let s = set_with(material.with_projection(Projection::uv(ImagePlacement { tiling: [2.0, 1.0], ..ImagePlacement::default() })));
    assert!(close(&row(&s, Compositor::new(&s)), &[0.9; 4]));
    assert_eq!(Compositor::new(&s).warnings(R), vec![CompositeWarning::MaterialFillSkipped { layer: "img".into() }]);
    let chain = ChainedSources { first: &NoFillSources, then: &UAsValue };
    let context = FillContext::with_sources(&chain);
    let c = Compositor::new(&s).with_fills(&context);
    assert!(close(&row(&s, c), &[-0.25, 0.25, 0.75, 1.25]));
    assert!(c.warnings(R).is_empty());
}
