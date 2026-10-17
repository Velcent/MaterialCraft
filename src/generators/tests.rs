//! The generators through the compositor, on a small set with synthetic baked maps: left half occluded and
//! concave, right half open with a convex edge band.

use super::*;
use crate::paint::effect::{EffectKind, GreyBlend, MaskEffect, Procedural};
use crate::paint::{BakedKind, BakedMap, BlendMode, Channel, Compositor, Fill, Layer, LayerKind, Mask, PixelFormat, TextureSet, TiledImage};

const R: Channel = Channel::Roughness;
const N: u32 = 32;

fn grey_map(kind: BakedKind, f: impl Fn(u32, u32) -> f32) -> BakedMap {
    let mut image = TiledImage::new(N, N, PixelFormat::from_id("gray16").unwrap()).unwrap();
    for y in 0..N {
        for x in 0..N {
            let v = f(x, y);
            image.set(x, y, [v, v, v, 1.0]);
        }
    }
    BakedMap { kind, image }
}

/// A set whose only layer is white roughness under a mask holding `generator` with `params`.
fn masked(id: &str, name: &str, params: &[(&str, &str)]) -> TextureSet {
    let mut set = TextureSet::new(id, name, (N, N)).with_channel(R);
    set.baked.push(grey_map(BakedKind::AmbientOcclusion, |x, _| if x < N / 2 { 0.1 } else { 1.0 }));
    set.baked.push(grey_map(BakedKind::Curvature, |x, _| {
        if x < N / 2 {
            0.2
        } else if x > 3 * N / 4 {
            0.95
        } else {
            0.5
        }
    }));
    let p = Procedural::new(name).with_params(params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
    let mut layer = Layer::new("l", "L", LayerKind::Fill(Fill::uniform(vec![(R, [1.0; 4])]))).with_blend(R, BlendMode::Normal, 1.0);
    layer.mask = Some(Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Generator(p))));
    set.layers.push(layer);
    set
}

/// Mean roughness over columns `x0..x1` (the mask value, as the layer is white over the default 0.5 below).
fn mean(set: &TextureSet, x0: u32, x1: u32) -> f32 {
    let (tex, warnings) = Compositor::with_effects(set, builtin()).composite_tex(R);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut sum = 0.0;
    for y in 0..N as usize {
        for x in x0 as usize..x1 as usize {
            sum += tex.px[y * N as usize + x][0];
        }
    }
    sum / ((x1 - x0) * N) as f32
}

#[test]
fn every_generator_is_registered_and_listed_once() {
    let r = builtin();
    for g in GENERATORS {
        assert!(r.generator(g.id).is_some(), "{}", g.id);
        assert_eq!(GENERATORS.iter().filter(|o| o.id == g.id).count(), 1);
    }
    assert!(generator("nope").is_none());
    assert!(MASK_STARTS.iter().all(|(id, _)| r.generator(id).is_some()));
}

#[test]
fn dirt_sits_in_occluded_cavities() {
    let set = masked("g-dirt", DIRT, &[("grunge_amount", "0")]);
    let (dirty, clean) = (mean(&set, 0, N / 2), mean(&set, N / 2, N));
    assert!(dirty > 0.9 && clean < 0.6, "{dirty} {clean}");
}

#[test]
fn edge_wear_sits_on_convex_edges() {
    let set = masked("g-wear", METAL_EDGE_WEAR, &[("grunge_amount", "0"), ("smoothness", "0")]);
    let (edge, cavity) = (mean(&set, 7 * N / 8, N), mean(&set, 0, N / 4));
    assert!(edge > 0.9 && cavity < 0.55, "{edge} {cavity}");
}

#[test]
fn occlusion_curvature_editor_and_builder_follow_their_maps() {
    let ao = masked("g-ao", AMBIENT_OCCLUSION, &[("invert", "1")]);
    assert!(mean(&ao, 0, N / 4) > mean(&ao, N / 2, N));
    let curv = masked("g-curv", CURVATURE, &[("curv_mode", "2")]);
    assert!(mean(&curv, 7 * N / 8, N) > mean(&curv, 0, N / 4));
    let editor = masked("g-editor", MASK_EDITOR, &[("ao_opacity", "1"), ("ao_blend", "0"), ("ao_invert", "1")]);
    assert!(mean(&editor, 0, N / 4) > 0.9 && mean(&editor, N / 2, 3 * N / 4) < 0.6);
    let builder = masked("g-builder", MASK_BUILDER, &[("grunge", "0"), ("curvature", "0"), ("ao_noise", "0")]);
    assert!(mean(&builder, 0, N / 4) > mean(&builder, N / 2, 3 * N / 4));
}

#[test]
fn uv_border_distance_is_white_at_island_borders() {
    let mut set = masked("g-uv", UV_BORDER_DISTANCE, &[("distance", "0.5")]);
    // One island in the middle of the texture: position is black outside it.
    set.baked.push(grey_map(BakedKind::Position, |x, y| if (8..24).contains(&x) && (8..24).contains(&y) { 0.5 } else { 0.0 }));
    let (tex, _) = Compositor::with_effects(&set, builtin()).composite_tex(R);
    let at = |x: usize, y: usize| tex.px[y * N as usize + x][0];
    assert!(at(8, 16) > at(15, 16), "{} {}", at(8, 16), at(15, 16));
}

#[test]
fn height_combination_through_the_layer_panel_start() {
    use crate::paint::edit::ops::MaskStart;
    let mut set = TextureSet::new("g-height", "S", (4, 1)).with_channel(R).with_channel(Channel::Height);
    let fill = |v: f32, r: f32| Fill::uniform(vec![(Channel::Height, [v, v, v, 1.0]), (R, [r, r, r, 1.0])]);
    let layer =
        |id: &str, f: Fill| Layer::new(id, id, LayerKind::Fill(f)).with_blend(R, BlendMode::Normal, 1.0).with_blend(Channel::Height, BlendMode::Normal, 1.0);
    set.layers.push(layer("below", fill(0.2, 0.0)));
    let mut top = layer("top", fill(0.8, 1.0));
    top.mask = Some(Mask::new(0.0).with_effect(MaskEffect::new(
        GreyBlend::default(),
        EffectKind::Generator(Procedural::new(match MaskStart::Generator(HEIGHT_COMBINATION) {
            MaskStart::Generator(id) => id,
            _ => "",
        })),
    )));
    set.layers.push(top);
    let (tex, warnings) = Compositor::with_effects(&set, builtin()).composite_tex(R);
    assert!(warnings.is_empty());
    assert!(tex.px[0][0] > 0.9, "{:?}", tex.px[0]);
    set.layers[0] = layer("below", fill(0.95, 0.0));
    let (tex, _) = Compositor::with_effects(&set, builtin()).composite_tex(R);
    assert!(tex.px[0][0] < 0.4, "{:?}", tex.px[0]);
}

#[test]
fn generators_name_the_baked_maps_they_read() {
    for info in GENERATORS {
        assert_eq!(baked_maps(info.id).is_empty(), info.id == HEIGHT_COMBINATION, "{}", info.id);
    }
    assert!(baked_maps("substance:mg_dirt").is_empty());
}
