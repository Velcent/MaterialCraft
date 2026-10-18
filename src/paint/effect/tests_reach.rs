use super::*;
use crate::paint::effect::{Anchor, Levels, MaskEffect, Procedural, Reference};
use crate::paint::{BlendMode, Compositor, ContentEffect, Fill, GreyBlend, Layer, LayerKind, Mask, PaintContent, PixelFormat, TiledImage};

const N: u32 = 32;

#[test]
fn union_then_and_growth() {
    assert_eq!(Reach::radius(0), Reach::Texel);
    assert_eq!(Reach::Texel.union(Reach::Radius(3)), Reach::Radius(3));
    assert_eq!(Reach::Radius(5).union(Reach::Radius(3)), Reach::Radius(5));
    assert_eq!(Reach::Radius(5).union(Reach::Whole), Reach::Whole);
    assert_eq!(Reach::Radius(5).then(Reach::Radius(3)), Reach::Radius(8));
    assert_eq!(Reach::Texel.then(Reach::Texel), Reach::Texel);
    assert_eq!(Reach::Radius(u32::MAX).then(Reach::Radius(1)), Reach::Whole);
    assert_eq!(Reach::Whole.texels(), None);
    let r = Rect { x: 10, y: 2, w: 4, h: 4 };
    assert_eq!(Reach::Texel.grow(r, (16, 16)), r);
    assert_eq!(Reach::Radius(3).grow(r, (16, 16)), Rect { x: 7, y: 0, w: 9, h: 9 });
    assert_eq!(Reach::Whole.grow(r, (16, 16)), Rect { x: 0, y: 0, w: 16, h: 16 });
    // empty or outside the image: nothing to grow
    assert_eq!(Reach::Radius(3).grow(Rect { x: 4, y: 4, w: 0, h: 2 }, (16, 16)).w, 0);
    assert_eq!(Reach::Whole.grow(Rect { x: 20, y: 4, w: 2, h: 2 }, (16, 16)).w, 0);
    // a rectangle reaching past the image is clipped first
    assert_eq!(Reach::Radius(1).grow(Rect { x: 14, y: 14, w: 9, h: 9 }, (16, 16)), Rect { x: 13, y: 13, w: 3, h: 3 });
}

#[test]
fn every_kind_reports_its_reach() {
    let registry = crate::generators::builtin();
    let set = TextureSet::new("s", "S", (2048, 2048));
    let m = EvalTarget::Mask;
    let grey = |kind| MaskEffect::new(GreyBlend::default(), kind);
    let blur = grey(EffectKind::Filter(Procedural::new("blur")));
    assert!(matches!(blur.reach(registry, &set, m).input, Reach::Radius(r) if r > 4));
    assert!(blur.reach(&EffectRegistry::new(), &set, m).is_local(), "unregistered: left out");
    let mut off = blur.clone();
    off.enabled = false;
    assert!(off.reach(registry, &set, m).is_local());
    assert!(grey(EffectKind::Filter(Procedural::new("invert"))).reach(registry, &set, m).is_local());
    let heights = grey(EffectKind::Generator(Procedural::new("height_combination"))).reach(registry, &set, m);
    assert_eq!((heights.input, heights.stack_height), (Reach::Texel, Some(Reach::Texel)));
    assert!(grey(EffectKind::Generator(Procedural::new("dirt"))).reach(registry, &set, m).is_local());
    let reference = grey(EffectKind::Reference(Reference::new("a"))).reach(registry, &set, m);
    assert_eq!(reference.anchor, Some(AnchorRead { anchor: "a".into(), channel: None }));
    assert!(!reference.is_local() && reference.input.is_texel());
    for kind in [EffectKind::Fill(0.5), EffectKind::Levels(Levels::default()), EffectKind::Anchor(Anchor { id: "a".into() })] {
        assert!(grey(kind).reach(registry, &set, m).is_local());
    }
}

fn grey_image(f: impl Fn(u32, u32) -> f32) -> TiledImage {
    let mut img = TiledImage::new(N, N, PixelFormat::from_id("graya16").unwrap()).unwrap();
    for y in 0..N {
        for x in 0..N {
            let v = f(x, y);
            img.set(x, y, [v, v, v, 1.0]);
        }
    }
    img
}

/// Paint, levels, an anchor, a fill and references to the anchor (in content and in a mask): every one of them is
/// `Texel`, so changing one painted texel changes the composite there and nowhere else.
fn native_set(paint: TiledImage) -> TextureSet {
    let c = Channel::Roughness;
    let levels = Levels { input_min: [0.1; 3], gamma: [1.7; 3], output_max: [0.9; 3], ..Levels::default() };
    let mut a = Layer::new("a", "A", LayerKind::Paint(PaintContent::from_images(vec![(c, paint)]))).with_blend(c, BlendMode::Normal, 1.0);
    a.effects.push(ContentEffect::new(Vec::new(), EffectKind::Levels(levels)).with_blend(c, BlendMode::Normal, 1.0));
    a.effects.push(ContentEffect::new(Vec::new(), EffectKind::Anchor(Anchor { id: "anc".into() })).with_blend(c, BlendMode::Normal, 1.0));
    let mut b = Layer::new("b", "B", LayerKind::Fill(Fill::uniform(vec![(c, [0.3; 4])]))).with_blend(c, BlendMode::Normal, 0.8);
    let read = Reference { anchor: "anc".into(), channel: Some(c), levels: Some(levels) };
    b.effects.push(ContentEffect::new(Vec::new(), EffectKind::Reference(read.clone())).with_blend(c, BlendMode::Multiply, 0.7));
    let grey = |kind| MaskEffect::new(GreyBlend::default(), kind);
    b.mask = Some(Mask::new(0.2).with_effect(grey(EffectKind::Reference(read))).with_effect(grey(EffectKind::Levels(levels))));
    let mut set = TextureSet::new("s", "S", (N, N)).with_channel(c);
    set.layers.push(a);
    set.layers.push(b);
    set
}

#[test]
fn native_kinds_change_only_the_texel_they_read() {
    let registry = crate::generators::builtin();
    let base = |x: u32, y: u32| ((x * 7 + y * 13) % 17) as f32 / 16.0;
    let before = native_set(grey_image(base));
    for layer in &before.layers {
        for e in &layer.effects {
            assert!(e.reach(registry, &before, EvalTarget::Channel(Channel::Roughness)).input.is_texel());
        }
    }
    let (k, c) = ((9, 4), (10, 5));
    let after = native_set(grey_image(|x, y| if (x, y) == k { 1.0 - base(x, y) } else { base(x, y) }));
    let (old, _) = Compositor::new(&before).composite_tex(Channel::Roughness);
    let (new, _) = Compositor::new(&after).composite_tex(Channel::Roughness);
    for y in 0..N {
        for x in 0..N {
            let i = (y * N + x) as usize;
            assert_eq!((x, y) == k, old.px[i] != new.px[i], "at {x},{y}");
        }
    }
    // the same over a single texel's rectangle
    let one = |set: &TextureSet| {
        let mut out = [[0.0; 4]];
        Compositor::new(set).composite_rect(Channel::Roughness, Rect { x: c.0, y: c.1, w: 1, h: 1 }, &mut out);
        out[0]
    };
    assert_eq!(one(&before), one(&after));
}
