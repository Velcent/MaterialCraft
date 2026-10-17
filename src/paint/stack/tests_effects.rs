//! Content effect stacks, mask stacks and registered generators/filters.

use super::*;
use crate::paint::effect::{EvalRequest, FilterInput, FilterKernel, GeneratorSource, Procedural};
use crate::paint::{ContentEffect, EffectKind, Fill, GeometryMask, GreyBlend, Mask, MaskEffect, PaintContent, PixelFormat};

const R: Channel = Channel::Roughness;

fn set(w: u32, h: u32) -> TextureSet {
    TextureSet::new("s", "S", (w, h)).with_channel(R)
}

fn fill_layer(id: &str, v: f32) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(R, [v, v, v, 1.0])]))).with_blend(R, BlendMode::Normal, 1.0)
}

fn fill_effect(v: f32, mode: BlendMode, opacity: f32) -> ContentEffect {
    ContentEffect::new(Vec::new(), EffectKind::Fill(Fill::uniform(vec![(R, [v, v, v, 1.0])]))).with_blend(R, mode, opacity)
}

fn grey(mode: BlendMode, opacity: f32, kind: EffectKind<crate::paint::effect::Grey>) -> MaskEffect {
    MaskEffect::new(GreyBlend { mode, opacity }, kind)
}

fn px(c: &Compositor, x: usize, y: usize) -> [f32; 4] {
    let (tex, _) = c.composite_tex(R);
    tex.px[y * tex.w + x]
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

/// Writes the x coordinate of each texel.
struct Ramp;
impl GeneratorSource for Ramp {
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let r = request.rect;
        for (i, p) in out.iter_mut().enumerate() {
            let x = (r.x + i as u32 % r.w) as f32 / 10.0;
            *p = [x, x, x, 1.0];
        }
        Ok(())
    }
}

/// Each texel becomes its left neighbour (clamped): needs a radius of 1.
struct ShiftRight;
impl FilterKernel for ShiftRight {
    fn radius(&self, _: &[(String, String)]) -> u32 {
        1
    }
    fn apply(&self, request: &EvalRequest, input: &FilterInput, out: &mut [[f32; 4]]) -> Result<(), String> {
        let (r, i) = (request.rect, input.rect);
        for (k, p) in out.iter_mut().enumerate() {
            let (x, y) = (r.x + k as u32 % r.w, r.y + k as u32 / r.w);
            let sx = x.saturating_sub(1).max(i.x);
            *p = input.pixels[((y - i.y) * i.w + (sx - i.x)) as usize];
        }
        Ok(())
    }
}

#[test]
fn content_effects_blend_over_the_base() {
    let mut s = set(2, 2);
    s.layers.push(fill_layer("base", 0.8).with_effect(fill_effect(0.5, BlendMode::Multiply, 1.0)));
    assert!(close(px(&Compositor::new(&s), 0, 0)[0], 0.4));
    // A switched-off effect and an effect without an entry for the channel change nothing.
    s.layers[0].effects[0].enabled = false;
    s.layers[0].effects.push(fill_effect(0.1, BlendMode::Normal, 1.0).with_blend(R, BlendMode::Disable, 1.0));
    assert!(close(px(&Compositor::new(&s), 0, 0)[0], 0.8));
}

#[test]
fn an_effect_on_empty_content_keeps_its_value_with_its_coverage() {
    let mut s = set(2, 2);
    s.layers.push(fill_layer("below", 0.2));
    let empty = Layer::new("fx", "FX", LayerKind::Paint(PaintContent::default())).with_blend(R, BlendMode::Normal, 1.0);
    s.layers.push(empty.with_effect(fill_effect(1.0, BlendMode::Multiply, 0.5)));
    // Content = 1.0 at coverage 0.5 (the effect's mode does not apply to nothing); normal over 0.2.
    let p = px(&Compositor::new(&s), 1, 1);
    assert!(close(p[0], 0.6) && close(p[3], 1.0), "{p:?}");
}

#[test]
fn mask_stacks_scale_the_layer() {
    let mut s = set(4, 1);
    s.layers.push(fill_layer("below", 0.0));
    let mut top = fill_layer("top", 1.0);
    let mut paint = TiledImage::new(4, 1, PixelFormat::from_id("graya8").unwrap()).unwrap();
    paint.set(1, 0, [1.0; 4]);
    paint.set(2, 0, [1.0, 1.0, 1.0, 0.0]);
    top.mask = Some(Mask::new(0.0).with_effect(grey(BlendMode::Normal, 1.0, EffectKind::Paint(crate::paint::MaskPixels::new(paint)))).with_effect(grey(
        BlendMode::Add,
        0.5,
        EffectKind::Fill(0.5),
    )));
    s.layers.push(top);
    let c = Compositor::new(&s);
    assert!(close(px(&c, 0, 0)[0], 0.25)); // 0 + 0.5 × 0.5
    assert!(close(px(&c, 1, 0)[0], 1.0)); // painted 1, + 0.25, clamped
    assert!(close(px(&c, 2, 0)[0], 0.25)); // no coverage
    // A disabled mask applies the layer fully; a constant one scales it uniformly.
    s.layers[1].mask.as_mut().unwrap().enabled = false;
    assert!(close(px(&Compositor::new(&s), 0, 0)[0], 1.0));
    s.layers[1].mask = Some(Mask::new(0.3));
    assert!(close(px(&Compositor::new(&s), 3, 0)[0], 0.3));
    s.layers[1].mask = Some(Mask::new(0.0));
    assert!(close(px(&Compositor::new(&s), 3, 0)[0], 0.0));
}

#[test]
fn image_masks_keep_red_times_alpha() {
    let mut s = set(2, 1);
    let mut img = TiledImage::new(2, 1, PixelFormat::from_id("graya16").unwrap()).unwrap();
    img.set(0, 0, [0.5, 0.5, 0.5, 0.5]);
    s.layers.push(fill_layer("f", 1.0).with_mask(img));
    let c = Compositor::new(&s);
    let p = px(&c, 0, 0);
    assert!(close(p[0], 0.5 + 0.5 * 0.25) && close(p[3], 0.25), "{p:?}");
    assert!(close(px(&c, 1, 0)[3], 0.0));
}

#[test]
fn generators_and_filters_come_from_the_registry() {
    let mut s = set(10, 1);
    let generator = ContentEffect::new(Vec::new(), EffectKind::Generator(Procedural::new("ramp"))).with_blend(R, BlendMode::Normal, 1.0);
    let filter = ContentEffect::new(Vec::new(), EffectKind::Filter(Procedural::new("shift"))).with_blend(R, BlendMode::Normal, 1.0);
    s.layers.push(fill_layer("l", 0.9).with_effect(generator).with_effect(filter));
    let plain = Compositor::new(&s);
    assert!(close(px(&plain, 5, 0)[0], 0.9), "unregistered effects are left out");
    let w = plain.warnings(R);
    assert_eq!(w.len(), 2);
    assert!(matches!(&w[0], CompositeWarning::EffectNotComputable { in_mask: false, index: 0, kind, .. } if kind == "generator"));
    let mut registry = EffectRegistry::new();
    registry.add_generator("ramp", Box::new(Ramp));
    registry.add_filter("shift", Box::new(ShiftRight));
    let c = Compositor::with_effects(&s, &registry);
    assert!(c.warnings(R).is_empty());
    assert!(close(px(&c, 5, 0)[0], 0.4), "{:?}", px(&c, 5, 0));
    assert!(close(px(&c, 0, 0)[0], 0.0));
    // Tiles far from the left edge read their neighbour through the padded input.
    let mut wide = set(300, 1);
    wide.layers = s.layers.clone();
    let (tex, _) = Compositor::with_effects(&wide, &registry).composite_tex(R);
    assert!(close(tex.px[256][0], 25.5), "{}", tex.px[256][0]);
}

#[test]
fn mask_generators_and_filters() {
    let mut s = set(10, 1);
    let mut l = fill_layer("l", 1.0);
    l.mask = Some(Mask::new(0.0).with_effect(grey(BlendMode::Normal, 1.0, EffectKind::Generator(Procedural::new("ramp")))).with_effect(grey(
        BlendMode::Normal,
        1.0,
        EffectKind::Filter(Procedural::new("shift")),
    )));
    s.layers.push(l);
    assert!(close(px(&Compositor::new(&s), 5, 0)[0], 0.5), "default value under a zero mask");
    let mut registry = EffectRegistry::new();
    registry.add_generator("ramp", Box::new(Ramp));
    registry.add_filter("shift", Box::new(ShiftRight));
    let c = Compositor::with_effects(&s, &registry);
    assert!(close(px(&c, 5, 0)[3], 0.4));
}

#[test]
fn group_effects_and_data_only_parts_are_reported() {
    let mut s = set(2, 2);
    s.layers.push(fill_layer("base", 0.8));
    let mut g = Layer::new("g", "G", LayerKind::Group(vec![fill_layer("in", 0.5)])).with_blend(R, BlendMode::Normal, 1.0);
    g.effects.push(fill_effect(0.5, BlendMode::Multiply, 1.0));
    s.layers.push(g);
    assert!(close(px(&Compositor::new(&s), 0, 0)[0], 0.25), "isolated group, then its effects");
    s.layers[1].set_blend(R, BlendMode::Passthrough, 1.0);
    s.layers[1].geometry_mask = Some(GeometryMask::Parts(vec!["a".into()]));
    let c = Compositor::new(&s);
    assert!(close(px(&c, 0, 0)[0], 0.5));
    let w = c.warnings(R);
    assert!(w.contains(&CompositeWarning::GroupEffectsSkipped { layer: "g".into() }), "{w:?}");
    assert!(w.contains(&CompositeWarning::GeometryMaskIgnored { layer: "g".into() }), "{w:?}");
}

/// Mask value 1 where the layer's own height is above the height below it, else 0.
struct Higher;
impl GeneratorSource for Higher {
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        let stack = request.stack.ok_or("no stack")?;
        let (own, below) = (stack.layer_height(request.rect), stack.height_below(request.rect));
        for ((o, h), b) in out.iter_mut().zip(own).zip(below) {
            let v = if h[1] > 0.0 && h[0] > b { 1.0 } else { 0.0 };
            *o = [v, v, v, 1.0];
        }
        Ok(())
    }
}

#[test]
fn mask_generators_see_the_height_of_their_layer_and_below() {
    let mut s = set(2, 1).with_channel(Channel::Height);
    let height = |id: &str, v: f32, r: f32| {
        Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(Channel::Height, [v, v, v, 1.0]), (R, [r, r, r, 1.0])])))
            .with_blend(Channel::Height, BlendMode::Normal, 1.0)
            .with_blend(R, BlendMode::Normal, 1.0)
    };
    s.layers.push(height("low", 0.3, 0.1));
    let mut top = height("top", 0.6, 0.9);
    top.mask = Some(Mask::new(0.0).with_effect(grey(BlendMode::Normal, 1.0, EffectKind::Generator(Procedural::new("higher")))));
    // Inside a pass-through folder, so the walk to the layer has to descend.
    let folder =
        Layer::new("g", "G", LayerKind::Group(vec![top])).with_blend(R, BlendMode::Passthrough, 1.0).with_blend(Channel::Height, BlendMode::Passthrough, 1.0);
    s.layers.push(folder);
    let mut registry = EffectRegistry::new();
    registry.add_generator("higher", Box::new(Higher));
    assert!(close(px(&Compositor::with_effects(&s, &registry), 0, 0)[0], 0.9), "0.6 is above 0.3: the mask lets the layer through");
    s.layers[0] = height("low", 0.8, 0.1);
    assert!(close(px(&Compositor::with_effects(&s, &registry), 0, 0)[0], 0.1), "0.6 is below 0.8");
}
