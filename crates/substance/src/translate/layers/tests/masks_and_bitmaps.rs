//! Masks made of uniform/bitmap fills, and bitmap fills from embedded images.

use super::fixtures::*;
use crate::model::*;
use crate::translate::layers::images::{ImageSource, LoadedImage};
use materialcraft::paint::{Channel, LayerKind, PixelFormat, PixelLayout, SampleType, TiledImage};

fn mask(initial: MaskInitial, effects: Vec<PainterEffect>) -> Option<PainterMask> {
    Some(PainterMask {
        enabled: true,
        initial,
        effects,
    })
}

fn mask_fill(source: PainterSource, mode: u64, opacity: f32) -> PainterEffect {
    PainterEffect::Fill(FillEffect {
        header: header(0, vec![blend(0, mode, opacity)]),
        sources: vec![source],
        projection: 0,
        filtering: 2,
        uv_transform: None,
    })
}

fn base_and(top: PainterLayer) -> Vec<PainterLayer> {
    vec![
        layer(1, "Base", vec![fill(vec![uniform(ROUGH, [0.0; 3])])], vec![]),
        top,
    ]
}

fn top(m: Option<PainterMask>) -> PainterLayer {
    let mut l = layer(2, "Top", vec![fill(vec![uniform(ROUGH, [1.0; 3])])], vec![]);
    l.mask = m;
    l
}

#[test]
fn plain_masks_become_constant_images_or_nothing() {
    let white = import(base_and(top(mask(MaskInitial::White, vec![]))));
    assert!(white.layers[1].mask.is_none(), "an all-white mask is no mask");
    let black = import(base_and(top(mask(MaskInitial::Black, vec![]))));
    assert!(black.layers[1].mask.is_some());
    assert!(composite(&black, Channel::Roughness, 2, 2)[0] < 1e-3);
    let quarter = mask_fill(uniform(0, [0.25; 3]), 2, 1.0);
    let grey = import(base_and(top(mask(MaskInitial::Black, vec![quarter]))));
    assert!((composite(&grey, Channel::Roughness, 2, 2)[0] - 0.25).abs() < 0.01);
    assert!(grey.verdict.all_exact());
    let off = PainterMask {
        enabled: false,
        ..mask(MaskInitial::Black, vec![]).unwrap()
    };
    assert!(import(base_and(top(Some(off)))).layers[1].mask.is_none());
}

#[test]
fn generator_masks_are_not_computable() {
    let generator = mask_fill(procedural(0, "mg_dirt"), 2, 1.0);
    let stack = import(base_and(top(mask(MaskInitial::Black, vec![generator]))));
    assert!(stack.layers[1].mask.is_none());
    let status = stack.verdict.channel(Channel::Roughness);
    assert_eq!(status.reasons(), ["mask generator \"mg_dirt\""]);
}

/// One 8×8 RGBA8 image whose left half is white.
struct HalfWhite;

impl ImageSource for HalfWhite {
    fn load(&self, url: &str) -> Result<LoadedImage, String> {
        if url != "/Logo?version=abc" {
            return Err(format!("no {url}"));
        }
        let mut image = TiledImage::new(8, 8, PixelFormat::new(PixelLayout::Rgba, SampleType::U8)).unwrap();
        image.fill([0.0, 0.0, 0.0, 1.0]);
        for y in 0..8 {
            for x in 0..4 {
                image.set(x, y, [1.0; 4]);
            }
        }
        Ok(LoadedImage { image, float: false })
    }
}

fn bitmap(bits: u64, url: &str) -> PainterSource {
    PainterSource::Bitmap(BitmapSource {
        uid: None,
        channels: ChannelMask(bits),
        url: url.into(),
    })
}

#[test]
fn bitmap_fills_become_paint_images_and_bitmap_masks_mask() {
    let logo = layer(
        3,
        "Logo",
        vec![fill(vec![bitmap(BASE, "/Logo?version=abc"), uniform(ROUGH, [0.7; 3])])],
        vec![],
    );
    let stack = import_with(vec![logo], &HalfWhite);
    let LayerKind::Paint(p) = &stack.layers[0].kind else {
        panic!("paint layer")
    };
    assert_eq!(p.images.len(), 2);
    assert!(stack.verdict.all_exact());
    assert!(composite(&stack, Channel::BaseColor, 1, 1)[0] > 0.99);
    assert!(composite(&stack, Channel::BaseColor, 6, 1)[0] < 0.01);

    let masked = top(mask(
        MaskInitial::Black,
        vec![mask_fill(bitmap(0, "/Logo?version=abc"), 2, 1.0)],
    ));
    let stack = import_with(base_and(masked), &HalfWhite);
    assert!(composite(&stack, Channel::Roughness, 1, 5)[0] > 0.99);
    assert!(composite(&stack, Channel::Roughness, 7, 5)[0] < 0.01);
}

#[test]
fn placed_or_missing_bitmaps_are_reported() {
    let mut placed = FillEffect {
        header: header(ALL, vec![]),
        sources: vec![bitmap(BASE, "/Logo?version=abc")],
        projection: 6,
        filtering: 2,
        uv_transform: Some(UvTransform {
            scale: [3.0, 3.0],
            translation: [0.0, 0.0],
            rotation: 0.0,
        }),
    };
    let stack = import_with(
        vec![layer(1, "Tri", vec![PainterEffect::Fill(placed.clone())], vec![])],
        &HalfWhite,
    );
    let status = stack.verdict.channel(Channel::BaseColor);
    assert!(status.is_computable() && status.reasons().len() == 2, "{status:?}");
    placed.sources = vec![bitmap(BASE, "resource://shelf/x?version=1")];
    let stack = import(vec![layer(1, "Shelf", vec![PainterEffect::Fill(placed)], vec![])]);
    assert!(!stack.verdict.channel(Channel::BaseColor).is_computable());
}
