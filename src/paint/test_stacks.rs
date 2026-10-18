//! Layer stacks for tests that compare incremental compositing with a full recompute: every blend mode family,
//! paint, fills, masks, folders (pass-through and isolated), effect layers, levels, baked maps, sizes that are not
//! a multiple of the tile size; and ([`reaching_stacks`]) generators, filters with a reach, anchors and references
//! around the painted layer. Deterministic (a small hash stands in for randomness).

use crate::paint::Mask;
use crate::paint::effect::{Anchor, EffectRegistry, EvalRequest, FilterInput, FilterKernel, GeneratorSource, Grey, Levels, Procedural, Reach, Reference};
use crate::paint::output::MaterialMaps;
use crate::paint::{
    BakedKind, BakedMap, BlendMode, Channel, ContentEffect, EffectKind, Fill, GreyBlend, Layer, LayerKind, MaskEffect, PaintContent, PixelFormat, PixelLayout,
    SampleType, TextureSet, TiledImage,
};
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Bit for bit, every map.
pub fn assert_same_maps(a: &MaterialMaps, b: &MaterialMaps, what: &str) {
    for c in Channel::STANDARD {
        let (Some(x), Some(y)) = (a.map(c), b.map(c)) else { continue };
        assert_eq!((x.w, x.h), (y.w, y.h));
        let diff = x.px.iter().zip(&y.px).position(|(p, q)| p.map(f32::to_bits) != q.map(f32::to_bits));
        assert!(diff.is_none(), "{what}: {c:?} differs at texel {:?}", diff.map(|i| (i % x.w, i / x.w)));
    }
}

/// The images of the stack's paint layer `paint` (it may sit in a folder).
pub fn paint_images(s: &mut TextureSet) -> &mut Vec<(Channel, TiledImage)> {
    match s.layer_mut("paint").map(|l| &mut l.kind) {
        Some(LayerKind::Paint(p)) => &mut p.images,
        _ => panic!("no paint layer"),
    }
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

/// The height below the layer, as grey (a generator reading the stack); counts its calls.
pub struct HeightBelow(pub &'static AtomicUsize);
impl GeneratorSource for HeightBelow {
    fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
        Some(Reach::Texel)
    }
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let below = request.stack.map(|s| s.height_below(request.rect)).unwrap_or_default();
        for (o, h) in out.iter_mut().zip(below) {
            *o = [h, h, h, 1.0];
        }
        Ok(())
    }
}

/// A value per texel from its position and a seed parameter (a generator reading nothing but its parameters).
pub struct Pattern;
impl GeneratorSource for Pattern {
    fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
        None
    }
    fn samples_points(&self) -> bool {
        true
    }
    fn sample_points(&self, params: &[(String, String)], taps: &[crate::projection::Tap], out: &mut [[f32; 4]]) -> Result<(), String> {
        let seed: u32 = params.iter().find(|(k, _)| k == "seed").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        for (t, o) in taps.iter().zip(out.iter_mut()) {
            let v = noise(seed, (t.uv[0].fract().abs() * 40.0) as u32, (t.uv[1].fract().abs() * 30.0) as u32);
            *o = [v, v * 0.5, 1.0 - v, 1.0];
        }
        Ok(())
    }
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let seed: u32 = request.params.iter().find(|(k, _)| k == "seed").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        let r = request.rect;
        for (i, o) in out.iter_mut().enumerate() {
            let (x, y) = (r.x + i as u32 % r.w.max(1), r.y + i as u32 / r.w.max(1));
            let v = noise(seed, x / 7, y / 5);
            *o = [v, 1.0 - v, v * v, 1.0];
        }
        Ok(())
    }
}

/// A box blur of radius `.0` (clamped at the input's edges); `gain` multiplies.
pub struct Blur(pub u32);
impl FilterKernel for Blur {
    fn radius(&self, _: &EvalRequest) -> u32 {
        self.0
    }
    fn reach(&self, _: &EvalRequest) -> Reach {
        Reach::radius(self.0)
    }
    fn apply(&self, request: &EvalRequest, input: &FilterInput, out: &mut [[f32; 4]]) -> Result<(), String> {
        let gain: f32 = request.params.iter().find(|(k, _)| k == "gain").and_then(|(_, v)| v.parse().ok()).unwrap_or(1.0);
        let (r, i, d) = (request.rect, input.rect, self.0);
        for (k, p) in out.iter_mut().enumerate() {
            let (x, y) = (r.x + k as u32 % r.w, r.y + k as u32 / r.w);
            let mut sum = [0.0f32; 4];
            let mut n = 0.0;
            for yy in y.saturating_sub(d).max(i.y)..=(y + d).min(i.y + i.h - 1) {
                for xx in x.saturating_sub(d).max(i.x)..=(x + d).min(i.x + i.w - 1) {
                    let q = input.pixels[((yy - i.y) * i.w + (xx - i.x)) as usize];
                    (0..4).for_each(|c| sum[c] += q[c]);
                    n += 1.0;
                }
            }
            *p = sum.map(|s| s / n * gain);
        }
        Ok(())
    }
}

static CALLS: AtomicUsize = AtomicUsize::new(0);

/// The effects [`reaching_stacks`] use: `below` and `pattern` generators, `blur1` and `blur3` filters.
pub fn registry() -> EffectRegistry {
    let mut r = EffectRegistry::new();
    r.add_generator("below", Box::new(HeightBelow(&CALLS)));
    r.add_generator("pattern", Box::new(Pattern));
    r.add_filter("blur1", Box::new(Blur(1)));
    r.add_filter("blur3", Box::new(Blur(3)));
    r
}

fn content(kind: EffectKind<crate::paint::effect::Content>, c: Channel, mode: BlendMode) -> ContentEffect {
    ContentEffect::new(Vec::new(), kind).with_blend(c, mode, 1.0)
}

fn proc(id: &str) -> Procedural {
    Procedural::new(id)
}

/// A mask effect blended normally at full opacity.
pub fn grey_effect(kind: EffectKind<Grey>) -> MaskEffect {
    MaskEffect::new(GreyBlend { mode: BlendMode::Normal, opacity: 1.0 }, kind)
}

/// Stacks with everything that reads beyond a texel, around a paint layer `paint` inside a folder (see
/// [`registry`]): generators reading only their parameters, below and above the paint; an effect layer above the
/// paint with blurs stacked (reach 1 + 3) and an anchor after them; a blur on the folder's content; a mask reading
/// the height below with a blur after it; a mask and content referencing the anchor; a blur on a layer that reads
/// only itself.
pub fn reaching_stacks() -> Vec<TextureSet> {
    let mut out = Vec::new();
    for (k, mut s) in local_stacks().into_iter().enumerate() {
        let mut pattern = uniform("pattern", &[(Channel::Roughness, 0.4)], BlendMode::Overlay, 0.7);
        let seeded = Procedural { params: vec![("seed".into(), k.to_string())], ..proc("pattern") };
        pattern.effects.push(content(EffectKind::Generator(seeded), Channel::Roughness, BlendMode::Normal));
        s.layers.insert(1, pattern);
        let mut own = uniform("own", &[(Channel::Metallic, 0.6)], BlendMode::Normal, 0.5);
        own.effects.push(content(EffectKind::Generator(proc("pattern")), Channel::Metallic, BlendMode::Multiply));
        own.effects.push(content(EffectKind::Filter(proc("blur3")), Channel::Metallic, BlendMode::Normal));
        // an image fill (it varies over the texture, so the stack cache keeps it) with an image fill effect
        let mut image = crate::paint::ImageFill::default();
        image.set_image(Channel::Roughness, blobs(s.resolution, 80 + k as u32, rgba8()));
        let mut fill = Layer::new("imagefill", "Image fill", LayerKind::Fill(Fill::new(crate::paint::FillSource::Image(image.clone()))));
        fill = fill.with_blend(Channel::Roughness, BlendMode::Multiply, 0.8);
        fill.effects.push(content(EffectKind::Fill(Fill::new(crate::paint::FillSource::Image(image))), Channel::Roughness, BlendMode::Screen));
        s.layers.push(fill);
        // a procedural texture fill (kept by the stack cache)
        let texture = crate::paint::TextureFill::on(
            &[Channel::Roughness, Channel::Metallic],
            &Procedural { params: vec![("seed".into(), "5".into())], ..proc("pattern") },
        );
        let tex = Layer::new("texfill", "Texture fill", LayerKind::Fill(Fill::new(crate::paint::FillSource::Texture(texture))));
        s.layers.insert(2, tex.with_blend(Channel::Roughness, BlendMode::Normal, 0.6).with_blend(Channel::Metallic, BlendMode::Lighten, 0.5));
        s.layers.push(own);
        let mut fx = Layer::new("fx2", "Blurs", LayerKind::Paint(PaintContent::default()));
        for c in [Channel::BaseColor, Channel::Height] {
            fx = fx.with_blend(c, BlendMode::Passthrough, 0.8);
        }
        for (id, c) in [("blur1", Channel::BaseColor), ("blur3", Channel::BaseColor), ("blur1", Channel::Height)] {
            fx.effects.push(content(EffectKind::Filter(proc(id)), c, BlendMode::Normal));
        }
        fx.effects.push(ContentEffect::new(Vec::new(), EffectKind::Anchor(Anchor { id: "blurred".into() })));
        s.layers.push(fx);
        let mut heights = uniform("heights", &[(Channel::BaseColor, 0.9)], BlendMode::Normal, 1.0);
        heights.mask =
            Some(Mask::new(0.0).with_effect(grey_effect(EffectKind::Generator(proc("below")))).with_effect(grey_effect(EffectKind::Filter(proc("blur1")))));
        s.layers.push(heights);
        let mut refers = uniform("refers", &[(Channel::Emissive, 1.0)], BlendMode::Normal, 1.0);
        let reference = || Reference { anchor: "blurred".into(), channel: Some(Channel::BaseColor), levels: None };
        refers.mask = Some(Mask::new(0.0).with_effect(grey_effect(EffectKind::Reference(reference()))));
        refers.effects.push(content(EffectKind::Reference(reference()), Channel::Emissive, BlendMode::Multiply));
        s.layers.push(refers);
        if let Some(at) = s.layers.iter().position(|l| l.id == "paint") {
            let paint = s.layers.remove(at);
            let channels: Vec<Channel> = paint.blend.iter().map(|b| b.channel).collect();
            let mut folder = Layer::new("pfolder", "Paint folder", LayerKind::Group(vec![paint]));
            for c in channels {
                folder = folder.with_blend(c, BlendMode::Normal, 1.0);
            }
            folder.effects.push(content(EffectKind::Filter(proc("blur1")), Channel::BaseColor, BlendMode::Normal));
            s.layers.insert(at, folder);
        }
        out.push(s);
    }
    out
}
