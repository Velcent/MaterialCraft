use super::*;
use crate::paint::{Fill, FillSource, PaintContent, PixelFormat};

const R: Channel = Channel::Roughness;

fn set(w: u32, h: u32) -> TextureSet {
    TextureSet::new("s", "S", (w, h)).with_channel(R).with_channel(Channel::BaseColor)
}

fn fill(id: &str, v: f32, mode: BlendMode) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(R, [v, v, v, 1.0])]))).with_blend(R, mode, 1.0)
}

fn paint(id: &str, w: u32, h: u32, px: &[(u32, u32, f32, f32)], mode: BlendMode) -> Layer {
    let mut img = TiledImage::new(w, h, PixelFormat::from_id("graya16").unwrap()).unwrap();
    for &(x, y, v, a) in px {
        img.set(x, y, [v, v, v, a]);
    }
    let mut c = PaintContent::default();
    c.set_image(R, img);
    Layer::new(id, id, LayerKind::Paint(c)).with_blend(R, mode, 1.0)
}

fn at(set: &TextureSet, x: usize, y: usize) -> [f32; 4] {
    let (tex, _) = Compositor::new(set).composite_tex(R);
    tex.px[y * tex.w + x]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn empty_stack_is_default_with_no_coverage() {
    assert_eq!(at(&set(4, 4), 1, 1), [0.5, 0.5, 0.5, 0.0]);
}

#[test]
fn two_paint_layers_with_coverage() {
    let mut s = set(4, 4);
    s.layers.push(paint("a", 4, 4, &[(0, 0, 1.0, 1.0), (1, 0, 1.0, 0.5)], BlendMode::Normal));
    s.layers.push(paint("b", 4, 4, &[(0, 0, 0.5, 1.0)], BlendMode::Multiply));
    let p00 = at(&s, 0, 0);
    assert!(close(p00[0], 0.5) && close(p00[3], 1.0));
    let p10 = at(&s, 1, 0);
    assert!(close(p10[0], 0.75) && close(p10[3], 0.5));
    assert_eq!(at(&s, 3, 3), [0.5, 0.5, 0.5, 0.0]);
}

#[test]
fn mask_hidden_opacity_and_missing_entry() {
    let mut s = set(4, 4);
    let mut mask = TiledImage::new(4, 4, PixelFormat::from_id("gray8").unwrap()).unwrap();
    mask.set(0, 0, [1.0; 4]);
    mask.set(1, 0, [0.5; 4]);
    s.layers.push(fill("masked", 1.0, BlendMode::Normal).with_mask(mask));
    let mut hidden = fill("hidden", 0.0, BlendMode::Normal);
    hidden.visible = false;
    s.layers.push(hidden);
    let mut no_entry = fill("other", 0.0, BlendMode::Normal);
    no_entry.blend.clear();
    s.layers.push(no_entry);
    assert!(close(at(&s, 0, 0)[0], 1.0));
    assert!(close(at(&s, 1, 0)[0], 0.5 + 0.5 * (0.5f32 * 255.0).round() / 255.0));
    assert_eq!(at(&s, 2, 2), [0.5, 0.5, 0.5, 0.0]);

    let mut s = set(4, 4);
    let mut l = fill("half", 1.0, BlendMode::Normal);
    l.opacity = 0.5;
    l.set_blend(R, BlendMode::Normal, 0.5);
    s.layers.push(l);
    let p = at(&s, 2, 2);
    assert!(close(p[0], 0.625) && close(p[3], 0.25));
}

#[test]
fn uniform_fill_covers_everything_and_material_fill_warns() {
    let mut s = set(4, 4);
    s.layers.push(fill("base", 0.8, BlendMode::Normal));
    let material = Fill { source: FillSource::Material { path: "materials/rust.mca".into(), params: vec![] } };
    s.layers.push(Layer::new("rust", "Rust", LayerKind::Fill(material)).with_blend(R, BlendMode::Normal, 1.0));
    assert_eq!(at(&s, 3, 3), [0.8, 0.8, 0.8, 1.0]);
    let (_, warnings) = Compositor::new(&s).composite_tex(R);
    assert_eq!(warnings, vec![CompositeWarning::MaterialFillSkipped { layer: "rust".into() }]);
    assert!(Compositor::new(&s).warnings(Channel::BaseColor).is_empty());
}

#[test]
fn groups_passthrough_and_isolated() {
    let group = |mode: BlendMode| {
        let mut g = Layer::new("g", "G", LayerKind::Group(vec![fill("m", 0.5, BlendMode::Multiply)])).with_blend(R, mode, 1.0);
        g.opacity = 0.5;
        g
    };
    let mut s = set(2, 2);
    s.layers.push(fill("base", 0.8, BlendMode::Normal));
    s.layers.push(group(BlendMode::Passthrough));
    assert!(close(at(&s, 0, 0)[0], 0.6)); // 0.8 + (0.8 * 0.5 - 0.8) * 0.5
    s.layers[1] = group(BlendMode::Normal);
    assert!(close(at(&s, 0, 0)[0], 0.525)); // isolated: 0.5 * 0.5 = 0.25, then normal at 0.5 over 0.8
    s.layers[1].opacity = 1.0;
    s.layers[1].set_blend(R, BlendMode::Multiply, 1.0);
    assert!(close(at(&s, 0, 0)[0], 0.2)); // 0.8 * 0.25
}

#[test]
fn tiles_agree_across_entry_points() {
    let (w, h) = (300, 140);
    let mut s = set(w, h);
    s.layers.push(fill("base", 0.2, BlendMode::Normal));
    s.layers.push(paint("p", w, h, &[(0, 0, 1.0, 1.0), (299, 139, 1.0, 1.0), (150, 130, 0.9, 0.5)], BlendMode::Add));
    let c = Compositor::new(&s);
    let (tex, _) = c.composite_tex(R);
    let (img, _) = c.composite_image(R).unwrap();
    assert_eq!(img.format().id(), "graya8");
    for (x, y) in [(0, 0), (299, 139), (150, 130), (10, 10)] {
        let a = tex.px[y * w as usize + x];
        let b = img.get(x as u32, y as u32);
        assert!((0..4).all(|i| (a[i].min(1.0) - b[i]).abs() < 0.5 / 255.0 + 1e-3), "{x},{y}: {a:?} vs {b:?}");
    }
    assert!(close(tex.px[139 * w as usize + 299][0], 1.2));
    let mut target = TiledImage::new(w, h, PixelFormat::from_id("rgba32f").unwrap()).unwrap();
    c.update(R, &mut target, &[(2, 1)]);
    assert_eq!(target.allocated_tiles(), 1);
    assert!(close(target.get(299, 139)[0], 1.2));
}

const C: Channel = Channel::BaseColor;

fn color_paint(id: &str, rgb: [f32; 3], coverage: f32) -> Layer {
    let mut img = TiledImage::new(2, 2, PixelFormat::from_id("rgba32f").unwrap()).unwrap();
    img.set(0, 0, [rgb[0], rgb[1], rgb[2], coverage]);
    let mut c = PaintContent::default();
    c.set_image(C, img);
    Layer::new(id, id, LayerKind::Paint(c)).with_blend(C, BlendMode::Normal, 1.0)
}

fn color_at(set: &TextureSet) -> [f32; 4] {
    Compositor::new(set).composite_tex(C).0.px[0]
}

#[test]
fn srgb_channels_blend_in_linear_light() {
    use crate::paint::working_space::{linear_to_srgb as enc, srgb_to_linear as dec};
    let mut s = set(2, 2);
    s.layers.push(color_paint("red", [1.0, 0.0, 0.0], 0.5));
    s.layers.push(color_paint("blue", [0.0, 0.0, 1.0], 0.5));
    let d = dec(0.5); // the default, 0.5 sRGB
    let red = d + (1.0 - d) * 0.5;
    let expect = [enc(red * 0.5), enc(d * 0.25), enc(d * 0.25 + 0.5), 0.75];
    let p = color_at(&s);
    assert!((0..4).all(|i| close(p[i], expect[i])), "{p:?} vs {expect:?}");
    assert!(!close(p[0], 0.375)); // the sRGB-space result

    // Multiply and a uniform fill: values are decoded, multiplied, re-encoded.
    let mut s = set(2, 2);
    let fill_c = |v: f32, mode| Layer::new("f", "f", LayerKind::Fill(Fill::uniform(vec![(C, [v, v, v, 1.0])]))).with_blend(C, mode, 1.0);
    s.layers.push(fill_c(0.6, BlendMode::Normal));
    s.layers.push(fill_c(0.5, BlendMode::Multiply));
    assert!(close(color_at(&s)[1], enc(dec(0.6) * dec(0.5))));
    // Fully covered 8-bit values come back exactly.
    s.layers.pop();
    assert_eq!(color_at(&s)[0], 0.6);
}

#[test]
fn channel_space_follows_the_spec() {
    let mut s = set(2, 2);
    assert_eq!(Compositor::new(&s).working_space(R), WorkingSpace::Stored);
    assert_eq!(Compositor::new(&s).working_space(C), WorkingSpace::LinearLight);
    // Linear channels keep blending stored values (half coverage of 1.0 over 0.5).
    s.layers.push(paint("p", 2, 2, &[(0, 0, 1.0, 0.5)], BlendMode::Normal));
    assert!(close(at(&s, 0, 0)[0], 0.75));
    // A channel declared sRGB composites in linear light.
    s.channels[0].color_space = crate::paint::ColorSpace::Srgb;
    assert_eq!(Compositor::new(&s).working_space(R), WorkingSpace::LinearLight);
    let p = at(&s, 0, 0)[0];
    use crate::paint::working_space::{linear_to_srgb, srgb_to_linear};
    assert!(close(p, linear_to_srgb(srgb_to_linear(0.5) * 0.5 + 0.5)), "{p}");
}
