//! Builds with the mask cache against fresh builds, bit for bit, through a series of edits: of the layers below a
//! mask that reads the stack, of a layer a reference reads, of a filter's parameter, of unrelated layers.

use super::*;
use crate::paint::effect::{Anchor, EffectRegistry, Procedural, Reference};
use crate::paint::output::{MaterialMaps, OutputOptions};
use crate::paint::stack::NO_FILL_CONTEXT;
use crate::paint::test_stacks::{Blur, HeightBelow, assert_same_maps as assert_same, blobs, grey_effect, local_stacks, paint_disc, paint_images};
use crate::paint::{BlendMode, Channel, Fill, LayerKind, Mask, PixelFormat, PixelLayout, SampleType};
use std::sync::atomic::{AtomicUsize, Ordering};

static ANY: AtomicUsize = AtomicUsize::new(0);
static COUNTED: AtomicUsize = AtomicUsize::new(0);

fn registry() -> EffectRegistry {
    counting(&ANY)
}

fn counting(calls: &'static AtomicUsize) -> EffectRegistry {
    let mut r = EffectRegistry::new();
    r.add_generator("below", Box::new(HeightBelow(calls)));
    r.add_filter("blur", Box::new(Blur(1)));
    r
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
            .with_effect(grey_effect(EffectKind::Paint(crate::paint::MaskPixels::new(blobs(size, 70, ga8)))))
            .with_effect(grey_effect(EffectKind::Filter(Procedural::new("blur"))))
            .with_effect(grey_effect(EffectKind::Anchor(Anchor { id: "a1".into() }))),
    );
    s.layers.push(blurred);
    let mut heights = fill("heights", Channel::Roughness, 0.1);
    heights.mask = Some(Mask::new(0.0).with_effect(grey_effect(EffectKind::Generator(Procedural::new("below")))));
    s.layers.push(heights);
    let mut refers = fill("refers", Channel::Metallic, 1.0);
    refers.mask = Some(Mask::new(0.0).with_effect(grey_effect(EffectKind::Reference(Reference { anchor: "a1".into(), channel: None, levels: None }))));
    s.layers.push(refers);
    s
}

/// Builds `s` with the cache and fresh; they must agree.
fn check(cache: &StackCache, s: &TextureSet, reg: &EffectRegistry, what: &str) {
    let o = OutputOptions::default();
    let session = cache.session(s, 7, None);
    let cached = MaterialMaps::build_cached(s, &o, reg, &NO_FILL_CONTEXT, Some(&session));
    let fresh = MaterialMaps::build_with(s, &o, reg, &NO_FILL_CONTEXT);
    assert_same(&cached, &fresh, what);
}

fn paint_layer(s: &mut TextureSet) -> &mut crate::paint::TiledImage {
    &mut paint_images(s)[0].1
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
    let session = cache.session(&s, 7, None);
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
    let session = cache.session(&s, 8, None);
    assert!(session.mask_key("heights").is_some());
    let dup = s.layers[0].clone();
    s.layers.push(dup);
    assert!(cache.session(&s, 8, None).mask_key("heights").is_none(), "nothing is kept for sets with repeated ids");
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
        assert_eq!(cache.session(&s, 7, None).keeps_below(), Some(k));
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
    reads.mask = Some(Mask::new(0.0).with_effect(grey_effect(EffectKind::Reference(Reference { anchor: "a2".into(), channel: None, levels: None }))));
    s.layers.push(reads);
    check(&cache, &s, &reg, "first");
    s.layer_mut("dirt").expect("dirt").opacity = 0.1;
    check(&cache, &s, &reg, "a layer below the effect layer");
    paint_disc(paint_layer(&mut s), (150, 120), 40, 3);
    check(&cache, &s, &reg, "painted below the effect layer");
}

#[test]
fn kept_fills_follow_their_layer() {
    let reg = crate::paint::test_stacks::registry();
    for mut s in crate::paint::test_stacks::reaching_stacks() {
        let cache = StackCache::default();
        assert!(cache.session(&s, 7, None).fill_key("texfill").is_some(), "procedural texture fills are kept");
        assert!(cache.session(&s, 7, None).fill_key("imagefill").is_none(), "image fills through the UVs are cheap to read again");
        check(&cache, &s, &reg, "first");
        s.layer_mut("texfill").expect("fill").opacity = 0.3;
        check(&cache, &s, &reg, "the fill's opacity");
        let Some(LayerKind::Fill(f)) = s.layer_mut("texfill").map(|l| &mut l.kind) else { panic!("fill") };
        let crate::paint::FillSource::Texture(t) = &mut f.source else { panic!("texture fill") };
        t.texture_mut(Channel::Roughness).expect("texture").params = vec![("seed".into(), "6".into())];
        check(&cache, &s, &reg, "the fill's texture");
    }
}
