//! Builds with the mask cache against fresh builds, bit for bit, through a series of edits: of the layers below a
//! mask that reads the stack, of a layer a reference reads, of a filter's parameter, of unrelated layers.

use super::*;
use crate::paint::effect::{Anchor, EffectRegistry, EvalRequest, FilterInput, FilterKernel, GeneratorSource, Grey, Procedural, Reference};
use crate::paint::output::{MaterialMaps, OutputOptions};
use crate::paint::stack::NO_FILL_CONTEXT;
use crate::paint::test_stacks::{blobs, local_stacks, paint_disc};
use crate::paint::{BlendMode, Channel, Fill, GreyBlend, LayerKind, Mask, MaskEffect, PixelFormat, PixelLayout, SampleType};
use std::sync::atomic::{AtomicUsize, Ordering};

static ANY: AtomicUsize = AtomicUsize::new(0);
static COUNTED: AtomicUsize = AtomicUsize::new(0);

/// The height below the layer, as grey; counts its calls.
struct HeightBelow(&'static AtomicUsize);
impl GeneratorSource for HeightBelow {
    fn generate(&self, request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let below = request.stack.map(|s| s.height_below(request.rect)).unwrap_or_default();
        for (o, h) in out.iter_mut().zip(below) {
            *o = [h, h, h, 1.0];
        }
        Ok(())
    }
}

/// A 3×3 box blur; `radius` scales nothing, `gain` multiplies.
struct Blur;
impl FilterKernel for Blur {
    fn radius(&self, _: &EvalRequest) -> u32 {
        1
    }
    fn apply(&self, request: &EvalRequest, input: &FilterInput, out: &mut [[f32; 4]]) -> Result<(), String> {
        let gain: f32 = request.params.iter().find(|(k, _)| k == "gain").and_then(|(_, v)| v.parse().ok()).unwrap_or(1.0);
        let (r, i) = (request.rect, input.rect);
        for (k, p) in out.iter_mut().enumerate() {
            let (x, y) = (r.x + k as u32 % r.w, r.y + k as u32 / r.w);
            let mut sum = [0.0f32; 4];
            let mut n = 0.0;
            for yy in y.saturating_sub(1).max(i.y)..=(y + 1).min(i.y + i.h - 1) {
                for xx in x.saturating_sub(1).max(i.x)..=(x + 1).min(i.x + i.w - 1) {
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

fn registry() -> EffectRegistry {
    counting(&ANY)
}

fn counting(calls: &'static AtomicUsize) -> EffectRegistry {
    let mut r = EffectRegistry::new();
    r.add_generator("below", Box::new(HeightBelow(calls)));
    r.add_filter("blur", Box::new(Blur));
    r
}

fn grey(mode: BlendMode, kind: EffectKind<Grey>) -> MaskEffect {
    MaskEffect::new(GreyBlend { mode, opacity: 1.0 }, kind)
}

fn fill(id: &str, c: Channel, v: f32) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(c, [v, v * 0.7, v * 0.3, 1.0])]))).with_blend(c, BlendMode::Normal, 1.0)
}

/// A local stack with masks on top that the cache keeps: a blurred painted mask holding an anchor, a mask reading
/// the height below, a mask referencing the anchor.
fn stack(k: usize) -> TextureSet {
    let mut s = local_stacks().remove(k);
    let size = s.resolution;
    let ga8 = PixelFormat::new(PixelLayout::GrayAlpha, SampleType::U8);
    let mut blurred = fill("blurred", Channel::BaseColor, 0.9);
    blurred.mask = Some(
        Mask::new(0.0)
            .with_effect(grey(BlendMode::Normal, EffectKind::Paint(crate::paint::MaskPixels::new(blobs(size, 70, ga8)))))
            .with_effect(grey(BlendMode::Normal, EffectKind::Filter(Procedural::new("blur"))))
            .with_effect(grey(BlendMode::Normal, EffectKind::Anchor(Anchor { id: "a1".into() }))),
    );
    s.layers.push(blurred);
    let mut heights = fill("heights", Channel::Roughness, 0.1);
    heights.mask = Some(Mask::new(0.0).with_effect(grey(BlendMode::Normal, EffectKind::Generator(Procedural::new("below")))));
    s.layers.push(heights);
    let mut refers = fill("refers", Channel::Metallic, 1.0);
    refers.mask =
        Some(Mask::new(0.0).with_effect(grey(BlendMode::Normal, EffectKind::Reference(Reference { anchor: "a1".into(), channel: None, levels: None }))));
    s.layers.push(refers);
    s
}

fn assert_same(a: &MaterialMaps, b: &MaterialMaps, what: &str) {
    for c in Channel::STANDARD {
        let (Some(x), Some(y)) = (a.map(c), b.map(c)) else { continue };
        let diff = x.px.iter().zip(&y.px).position(|(p, q)| p.map(f32::to_bits) != q.map(f32::to_bits));
        assert!(diff.is_none(), "{what}: {c:?} differs at texel {diff:?}");
    }
}

/// Builds `s` with the cache and fresh; they must agree.
fn check(cache: &StackCache, s: &TextureSet, reg: &EffectRegistry, what: &str) {
    let o = OutputOptions::default();
    let session = cache.session(s, 7);
    let cached = MaterialMaps::build_cached(s, &o, reg, &NO_FILL_CONTEXT, Some(&session));
    let fresh = MaterialMaps::build_with(s, &o, reg, &NO_FILL_CONTEXT);
    assert_same(&cached, &fresh, what);
}

fn paint_layer(s: &mut TextureSet) -> &mut crate::paint::TiledImage {
    let Some(LayerKind::Paint(p)) = s.layer_mut("paint").map(|l| &mut l.kind) else { panic!("no paint layer") };
    &mut p.images[0].1
}

#[test]
fn cached_builds_match_fresh_ones_through_edits() {
    let reg = registry();
    for k in 0..local_stacks().len() {
        let cache = StackCache::default();
        let mut s = stack(k);
        check(&cache, &s, &reg, "first build");
        assert!(cache.bytes() > 0, "the masks were kept");
        // edits that the kept masks must follow
        paint_disc(paint_layer(&mut s), (60, 60), 25, 5);
        check(&cache, &s, &reg, "painted below the height mask");
        if let Some(LayerKind::Fill(_)) = s.layer("dirt").map(|l| &l.kind) {
            s.layer_mut("dirt").expect("dirt").opacity = 0.3;
            check(&cache, &s, &reg, "opacity of a layer below");
        }
        let m = s.layer_mut("blurred").and_then(|l| l.mask.as_mut()).expect("mask");
        if let EffectKind::Filter(p) = &mut m.effects[1].kind {
            p.params = vec![("gain".into(), "0.5".into())];
        }
        check(&cache, &s, &reg, "filter parameter (the reference follows)");
        // a layer below that wrote no height starts writing it: the height mask follows
        let k_heights = s.layers.iter().position(|l| l.id == "heights").expect("heights");
        let quiet = s.layers[..k_heights].iter().position(|l| l.blend_for(Channel::Height).is_none() && !matches!(l.kind, LayerKind::Group(_)));
        if let Some(q) = quiet {
            s.layers[q].opacity = 0.5;
            check(&cache, &s, &reg, "a layer without height changed");
            s.layers[q].set_blend(Channel::Height, BlendMode::Normal, 1.0);
            if let LayerKind::Fill(f) = &mut s.layers[q].kind {
                *f = Fill::uniform(vec![(Channel::Height, [0.95; 4]), (Channel::BaseColor, [0.3; 4])]);
            }
            check(&cache, &s, &reg, "the layer writes height now");
        }
        s.layers.swap(0, 1);
        check(&cache, &s, &reg, "layers reordered");
        s.layer_mut("refers").expect("refers").visible = false;
        check(&cache, &s, &reg, "a layer hidden");
    }
}

#[test]
fn an_unrelated_edit_reuses_the_kept_masks() {
    let reg = counting(&COUNTED);
    let cache = StackCache::default();
    let mut s = stack(0);
    check(&cache, &s, &reg, "first");
    let before = COUNTED.load(Ordering::Relaxed);
    s.layer_mut("refers").expect("refers").opacity = 0.4;
    let o = OutputOptions::default();
    let session = cache.session(&s, 7);
    let _ = MaterialMaps::build_cached(&s, &o, &reg, &NO_FILL_CONTEXT, Some(&session));
    // the height mask lies below the edited layer: none of its tiles is generated again
    assert_eq!(COUNTED.load(Ordering::Relaxed), before, "no tile of the height mask was generated again");
}

#[test]
fn duplicate_ids_or_a_new_context_keep_nothing_stale() {
    let reg = registry();
    let cache = StackCache::default();
    let mut s = stack(0);
    check(&cache, &s, &reg, "first");
    let session = cache.session(&s, 8);
    assert!(session.mask_key("heights").is_some());
    let dup = s.layers[0].clone();
    s.layers.push(dup);
    assert!(cache.session(&s, 8).mask_key("heights").is_none(), "nothing is kept for sets with repeated ids");
}

#[test]
fn repeated_edits_of_one_layer_start_from_the_kept_composite() {
    let reg = registry();
    let cache = StackCache::default();
    let mut s = stack(0);
    let k = s.layers.iter().position(|l| l.id == "heights").expect("heights");
    check(&cache, &s, &reg, "first");
    for (step, v) in [0.9f32, 0.6, 0.3, 0.75].into_iter().enumerate() {
        s.layers[k].opacity = v;
        check(&cache, &s, &reg, &format!("drag step {step}"));
        assert_eq!(cache.session(&s, 7).keeps_below(), Some(k));
    }
    // an edit below the kept composite makes it stale
    paint_disc(paint_layer(&mut s), (200, 150), 30, 8);
    check(&cache, &s, &reg, "painted below the kept composite");
    s.layers[k].opacity = 0.2;
    check(&cache, &s, &reg, "the layer again");
}

#[test]
fn references_into_an_effect_layer_follow_the_stack_below_it() {
    let reg = registry();
    let cache = StackCache::default();
    let mut s = stack(0);
    let mut fx = Layer::new("fxa", "FX", LayerKind::Paint(crate::paint::PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Passthrough, 1.0);
    fx.effects.push(crate::paint::ContentEffect::new(Vec::new(), EffectKind::Anchor(Anchor { id: "a2".into() })));
    s.layers.push(fx);
    let mut reads = fill("reads", Channel::Roughness, 0.8);
    reads.mask =
        Some(Mask::new(0.0).with_effect(grey(BlendMode::Normal, EffectKind::Reference(Reference { anchor: "a2".into(), channel: None, levels: None }))));
    s.layers.push(reads);
    check(&cache, &s, &reg, "first");
    s.layer_mut("dirt").expect("dirt").opacity = 0.1;
    check(&cache, &s, &reg, "a layer below the effect layer");
    paint_disc(paint_layer(&mut s), (150, 120), 40, 3);
    check(&cache, &s, &reg, "painted below the effect layer");
}
