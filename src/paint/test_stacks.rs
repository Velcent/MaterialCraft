//! Layer stacks for tests that compare incremental compositing with a full recompute: every blend mode family,
//! paint, fills, masks, folders (pass-through and isolated), effect layers, levels, baked maps, sizes that are not
//! a multiple of the tile size. Deterministic (a small hash stands in for randomness).

use crate::paint::effect::{Grey, Levels};
use crate::paint::{
    BakedKind, BakedMap, BlendMode, Channel, ContentEffect, EffectKind, Fill, GreyBlend, Layer, LayerKind, MaskEffect, PaintContent, PixelFormat, PixelLayout,
    SampleType, TextureSet, TiledImage,
};

/// Channels the stacks declare.
pub const CHANNELS: [Channel; 7] =
    [Channel::BaseColor, Channel::Roughness, Channel::Metallic, Channel::Normal, Channel::Height, Channel::AmbientOcclusion, Channel::Emissive];

/// A value 0..1 from integers.
pub fn noise(a: u32, b: u32, c: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA77) ^ c.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// An RGBA image of `size` with paint (alpha > 0) inside a few blobs only, so some tiles stay unallocated.
pub fn blobs(size: (u32, u32), seed: u32, format: PixelFormat) -> TiledImage {
    let mut img = TiledImage::new(size.0, size.1, format).expect("image");
    for k in 0..4 {
        let cx = (noise(seed, k, 1) * size.0 as f32) as i64;
        let cy = (noise(seed, k, 2) * size.1 as f32) as i64;
        let r = 10 + (noise(seed, k, 3) * 50.0) as i64;
        paint_disc(&mut img, (cx, cy), r, seed + k);
    }
    img
}

/// Paints a disc of noisy values with soft coverage.
pub fn paint_disc(img: &mut TiledImage, centre: (i64, i64), r: i64, seed: u32) {
    let (w, h) = (img.width() as i64, img.height() as i64);
    for y in (centre.1 - r).max(0)..(centre.1 + r).min(h) {
        for x in (centre.0 - r).max(0)..(centre.0 + r).min(w) {
            let d = (((x - centre.0).pow(2) + (y - centre.1).pow(2)) as f32).sqrt() / r as f32;
            if d < 1.0 {
                let v = [noise(seed, x as u32, y as u32), noise(seed + 1, x as u32, y as u32), noise(seed + 2, x as u32, y as u32), 1.0 - d];
                img.set(x as u32, y as u32, v);
            }
        }
    }
}

fn rgba8() -> PixelFormat {
    PixelFormat::new(PixelLayout::Rgba, SampleType::U8)
}

fn grey_alpha8() -> PixelFormat {
    PixelFormat::new(PixelLayout::GrayAlpha, SampleType::U8)
}

fn uniform(id: &str, values: &[(Channel, f32)], mode: BlendMode, opacity: f32) -> Layer {
    let fill = Fill::uniform(values.iter().map(|(c, v)| (*c, [*v, *v * 0.8, *v * 0.6, 1.0])).collect());
    let mut l = Layer::new(id, id, LayerKind::Fill(fill));
    for (c, _) in values {
        l = l.with_blend(*c, mode, opacity);
    }
    l
}

fn paint(id: &str, size: (u32, u32), seed: u32, channels: &[Channel], mode: BlendMode) -> Layer {
    let images = channels.iter().enumerate().map(|(k, c)| (*c, blobs(size, seed + 7 * k as u32, rgba8()))).collect();
    let mut l = Layer::new(id, id, LayerKind::Paint(PaintContent::from_images(images)));
    for c in channels {
        l = l.with_blend(*c, mode, 0.9);
    }
    l
}

fn masked(l: Layer, size: (u32, u32), seed: u32) -> Layer {
    let mut mask = blobs(size, seed, grey_alpha8());
    paint_disc(&mut mask, (size.0 as i64 / 2, size.1 as i64 / 2), 30, seed + 3);
    l.with_mask(mask)
}

fn levels() -> Levels {
    Levels { input_min: [0.1; 3], input_max: [0.9; 3], gamma: [1.3; 3], ..Levels::default() }
}

/// The base set of a stack: `size`, every channel of [`CHANNELS`], a uniform base fill.
fn base(size: (u32, u32)) -> TextureSet {
    let mut s = TextureSet::new("s", "S", size);
    for c in CHANNELS {
        s = s.with_channel(c);
    }
    s.layers.push(uniform(
        "base",
        &[(Channel::BaseColor, 0.5), (Channel::Roughness, 0.6), (Channel::Height, 0.4), (Channel::AmbientOcclusion, 1.0)],
        BlendMode::Normal,
        1.0,
    ));
    s
}

/// Several local stacks (see the module docs), each with a paint layer `paint` to paint into.
pub fn local_stacks() -> Vec<TextureSet> {
    let size = (300, 260);
    let all = [Channel::BaseColor, Channel::Roughness, Channel::Height, Channel::Metallic];
    let mut out = Vec::new();

    // paint over a fill, masked fills and pass-through folders around it
    let mut s = base(size);
    s.layers.push(paint("paint", size, 1, &all, BlendMode::Normal));
    s.layers.push(masked(uniform("dirt", &[(Channel::BaseColor, 0.2), (Channel::Height, 0.1)], BlendMode::Multiply, 0.7), size, 20));
    let inner = vec![
        paint("inner", size, 30, &[Channel::BaseColor, Channel::Height], BlendMode::Overlay),
        uniform("glow", &[(Channel::Emissive, 0.9)], BlendMode::Add, 0.5),
    ];
    let mut folder = Layer::new("folder", "Folder", LayerKind::Group(inner)).with_blend(Channel::BaseColor, BlendMode::Passthrough, 0.8);
    folder = folder.with_blend(Channel::Height, BlendMode::Passthrough, 0.8).with_blend(Channel::Emissive, BlendMode::Passthrough, 1.0);
    s.layers.push(masked(folder, size, 40));
    out.push(s);

    // an isolated folder with a levels effect, an effect layer, baked normal and AO
    let mut s = base(size);
    let kids = vec![
        uniform("under", &[(Channel::BaseColor, 0.3), (Channel::Roughness, 0.2)], BlendMode::Normal, 1.0),
        paint("paint", size, 2, &all, BlendMode::Screen),
    ];
    let mut group = Layer::new("iso", "Isolated", LayerKind::Group(kids)).with_blend(Channel::BaseColor, BlendMode::SoftLight, 0.9);
    group = group.with_blend(Channel::Roughness, BlendMode::Normal, 1.0).with_blend(Channel::Height, BlendMode::Normal, 1.0);
    group.effects.push(ContentEffect::new(Vec::new(), EffectKind::Levels(levels())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    s.layers.push(group);
    let mut fx = Layer::new("fx", "Levels on all", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Passthrough, 0.6);
    fx.effects.push(ContentEffect::new(Vec::new(), EffectKind::Levels(levels())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    s.layers.push(fx);
    let mut nmap = TiledImage::new(size.0, size.1, rgba8()).expect("image");
    nmap.fill([0.55, 0.45, 0.9, 1.0]);
    paint_disc(&mut nmap, (100, 100), 60, 9);
    let mut ao = TiledImage::new(size.0, size.1, grey_alpha8()).expect("image");
    ao.fill([0.8, 0.8, 0.8, 1.0]);
    paint_disc(&mut ao, (200, 50), 40, 11);
    s.baked.push(BakedMap { kind: BakedKind::Normal, image: nmap });
    s.baked.push(BakedMap { kind: BakedKind::AmbientOcclusion, image: ao });
    out.push(s);

    // a masked paint layer with a mask stack (paint, fill, levels), darken / linear light, a normal channel
    let mut s = base(size);
    s.layers.push(paint("under", size, 50, &[Channel::Normal, Channel::BaseColor], BlendMode::Normal));
    let mut p = paint("paint", size, 3, &all, BlendMode::LinearLight);
    let mut mask_paint = blobs(size, 60, grey_alpha8());
    paint_disc(&mut mask_paint, (150, 130), 70, 61);
    let mut l = p.clone().with_mask(mask_paint);
    if let Some(m) = l.mask.as_mut() {
        m.effects.push(MaskEffect::new(GreyBlend { mode: BlendMode::Multiply, opacity: 0.5 }, EffectKind::<Grey>::Fill(0.7)));
        m.effects.push(MaskEffect::new(GreyBlend { mode: BlendMode::Normal, opacity: 1.0 }, EffectKind::<Grey>::Levels(levels())));
    }
    p = l;
    s.layers.push(p);
    s.layers.push(uniform("top", &[(Channel::Roughness, 0.1), (Channel::Metallic, 1.0)], BlendMode::Darken, 0.4));
    out.push(s);
    out
}
