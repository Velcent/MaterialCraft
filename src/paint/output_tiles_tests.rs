//! Tile-by-tile maps against the whole build: the same bits, also after painting into a few tiles, through stacks
//! with generators, filters with a reach, anchors, references and folders, with and without the stack cache.

use super::*;
use crate::paint::effect::EffectRegistry;
use crate::paint::height_normal::HeightFilter;
use crate::paint::stack::{NO_FILL_CONTEXT, StackCache};
use crate::paint::test_stacks::{Blur, local_stacks, paint_disc, reaching_stacks, registry};
use crate::paint::{Layer, LayerKind};
use crate::tex::Tex;

fn options(filter: HeightFilter) -> OutputOptions {
    OutputOptions { height_filter: filter, ..OutputOptions::default() }
}

/// Every test stack with the registry its effects need.
fn stacks() -> Vec<(TextureSet, EffectRegistry)> {
    let local = local_stacks().into_iter().map(|s| (s, EffectRegistry::new()));
    local.chain(reaching_stacks().into_iter().map(|s| (s, registry()))).collect()
}

fn full(set: &TextureSet, o: &OutputOptions, reg: &EffectRegistry) -> MaterialMaps {
    let mut m = MaterialMaps::build_with(set, o, reg, &NO_FILL_CONTEXT);
    m.warnings.clear();
    m
}

fn tiles(set: &TextureSet, o: &OutputOptions, reg: &EffectRegistry, cache: Option<&CacheSession>, t: &[(u32, u32)]) -> Vec<TileMaps> {
    MaterialMaps::build_tiles(set, o, reg, &NO_FILL_CONTEXT, cache, t).expect("tile by tile")
}

fn all_tiles(set: &TextureSet) -> Vec<(u32, u32)> {
    let (gx, gy) = grid(set);
    (0..gy).flat_map(|y| (0..gx).map(move |x| (x, y))).collect()
}

/// Writes the tiles into a copy of `maps`.
fn patched(maps: &MaterialMaps, tiles: &[TileMaps]) -> MaterialMaps {
    let mut out = maps.clone();
    for t in tiles {
        for c in Channel::STANDARD {
            let (Some(src), Some(dst)) = (t.map(c), map_mut(&mut out, c)) else { continue };
            for (i, p) in src.iter().enumerate() {
                let (x, y) = (t.rect.x as usize + i % t.rect.w as usize, t.rect.y as usize + i / t.rect.w as usize);
                dst.px[y * dst.w + x] = *p;
            }
        }
    }
    out
}

fn map_mut(m: &mut MaterialMaps, c: Channel) -> Option<&mut Tex> {
    Some(match c {
        Channel::BaseColor => &mut m.base_color,
        Channel::Roughness => &mut m.roughness,
        Channel::Metallic => &mut m.metallic,
        Channel::Normal => &mut m.normal,
        Channel::Height => &mut m.height,
        Channel::AmbientOcclusion => &mut m.ambient_occlusion,
        Channel::Emissive => &mut m.emissive,
        Channel::Opacity => &mut m.opacity,
        Channel::User(_) => return None,
    })
}

/// Bit for bit, every map.
fn assert_same(a: &MaterialMaps, b: &MaterialMaps, what: &str) {
    for c in Channel::STANDARD {
        let (Some(x), Some(y)) = (a.map(c), b.map(c)) else { continue };
        assert_eq!((x.w, x.h), (y.w, y.h));
        let diff = x.px.iter().zip(&y.px).position(|(p, q)| p.map(f32::to_bits) != q.map(f32::to_bits));
        assert!(diff.is_none(), "{what}: {c:?} differs at texel {:?}", diff.map(|i| (i % x.w, i / x.w)));
    }
}

/// The paint layer's images (it may sit in a folder).
fn paint_images(s: &mut TextureSet) -> &mut Vec<(Channel, crate::paint::TiledImage)> {
    match s.layer_mut("paint").map(|l| &mut l.kind) {
        Some(LayerKind::Paint(p)) => &mut p.images,
        _ => panic!("no paint layer"),
    }
}

/// Paints a disc into every image of the paint layer; the tiles it changed.
fn stroke(s: &mut TextureSet, at: (i64, i64), r: i64, seed: u32) -> Vec<(u32, u32)> {
    let mut changed = Vec::new();
    for (_, img) in paint_images(s).iter_mut() {
        img.clear_dirty();
        paint_disc(img, at, r, seed);
        changed.extend(img.dirty_tiles());
    }
    changed.sort_unstable();
    changed.dedup();
    changed
}

#[test]
fn every_tile_matches_the_whole_build() {
    for filter in [HeightFilter::Sobel, HeightFilter::Backward] {
        for (k, (set, reg)) in stacks().iter().enumerate() {
            let o = options(filter);
            let whole = full(set, &o, reg);
            let rebuilt = patched(&MaterialMaps { ..whole.clone() }, &tiles(set, &o, reg, None, &all_tiles(set)));
            assert_same(&rebuilt, &whole, &format!("stack {k}, {filter:?}"));
        }
    }
}

#[test]
fn painting_a_few_tiles_then_updating_them_matches_a_full_recompute() {
    for (k, (set, reg)) in stacks().iter().enumerate() {
        let o = options(HeightFilter::Sobel);
        let before = full(set, &o, reg);
        let mut painted = set.clone();
        // across a tile corner, so four tiles change
        let changed = stroke(&mut painted, (128, 128), 20, 99 + k as u32);
        let reached = MaterialMaps::tiles_reached(&painted, &o, reg, &NO_FILL_CONTEXT, &["paint"], &changed).expect("tile by tile");
        assert!(reached.tiles.len() > changed.len(), "the normal reaches the neighbours");
        let updated = patched(&before, &tiles(&painted, &o, reg, None, &reached.tiles));
        assert_same(&updated, &full(&painted, &o, reg), &format!("stack {k}"));
    }
}

#[test]
fn the_changed_area_grows_through_every_filter_reading_the_paint() {
    // no normal from the height, so only the filters spread the change: a dab ending 2 texels before a tile edge
    // reaches the next tile through the blurs above it (1 + 3 on the effect layer, 1 on the folder, 1 in the mask)
    let o = OutputOptions { height_normal_strength: 0.0, ..OutputOptions::default() };
    let mut sensitive = 0;
    for (k, set) in reaching_stacks().iter().enumerate() {
        let reg = registry();
        let reach = Compositor::with_effects(set, &reg).reach(&["paint"]).expect("in the set");
        assert!(reach > 4, "stack {k}: reach {reach}");
        let before = full(set, &o, &reg);
        let mut painted = set.clone();
        let changed = stroke(&mut painted, (60, 60), 65, 7 + k as u32);
        assert!(changed.iter().all(|t| t.0 == 0 && t.1 == 0), "{changed:?}");
        let reached = MaterialMaps::tiles_reached(&painted, &o, &reg, &NO_FILL_CONTEXT, &["paint"], &changed).expect("tile by tile");
        assert!(reached.tiles.contains(&(1, 0)) && reached.tiles.contains(&(1, 1)), "grown into the neighbours: {:?}", reached.tiles);
        let updated = patched(&before, &tiles(&painted, &o, &reg, None, &reached.tiles));
        assert_same(&updated, &full(&painted, &o, &reg), &format!("stack {k}"));
        // too short a reach would leave the neighbour stale
        let short = patched(&before, &tiles(&painted, &o, &reg, None, &changed));
        let base = |m: &MaterialMaps| m.base_color.px.iter().map(|p| p.map(f32::to_bits)).collect::<Vec<_>>();
        sensitive += usize::from(base(&short) != base(&full(&painted, &o, &reg)));
    }
    assert!(sensitive > 0, "a stroke reaches across the tile edge in some stack, so a short reach would show");
}

#[test]
fn strokes_through_the_cache_keep_it_exact() {
    let o = options(HeightFilter::Backward);
    for (k, (set, reg)) in stacks().iter().enumerate() {
        let cache = StackCache::default();
        let mut s = set.clone();
        let mut shown = MaterialMaps::build_cached(&s, &o, reg, &NO_FILL_CONTEXT, Some(&cache.session(&s, 3, None)));
        shown.warnings.clear();
        for (step, at) in [(40i64, 50i64), (200, 150), (130, 240)].into_iter().enumerate() {
            let changed = stroke(&mut s, at, 18, step as u32);
            let reached = MaterialMaps::tiles_reached(&s, &o, reg, &NO_FILL_CONTEXT, &["paint"], &changed).expect("tile by tile");
            let session = cache.session(&s, 3, Some(&reached.area));
            shown = patched(&shown, &tiles(&s, &o, reg, Some(&session), &reached.tiles));
            assert_same(&shown, &full(&s, &o, reg), &format!("stack {k}, stroke {step}"));
        }
        // what the strokes left in the cache is exact for a whole build too
        let mut whole = MaterialMaps::build_cached(&s, &o, reg, &NO_FILL_CONTEXT, Some(&cache.session(&s, 3, None)));
        whole.warnings.clear();
        assert_same(&whole, &full(&s, &o, reg), &format!("stack {k}, whole build after the strokes"));
    }
}

#[test]
fn whole_set_reasons() {
    let (mut set, _) = stacks().remove(0);
    let o = OutputOptions::default();
    let mut wide = EffectRegistry::new();
    wide.add_filter("wide", Box::new(Blur(400)));
    let mut l =
        Layer::new("w", "W", LayerKind::Paint(crate::paint::PaintContent::default())).with_blend(Channel::BaseColor, crate::paint::BlendMode::Passthrough, 1.0);
    l.effects.push(crate::paint::ContentEffect::new(Vec::new(), crate::paint::EffectKind::Filter(crate::paint::effect::Procedural::new("wide"))));
    let mut far = set.clone();
    far.layers.push(l);
    let mut big = far.clone();
    big.resolution = (1024, 1024);
    for (_, img) in paint_images(&mut big).iter_mut() {
        *img = crate::paint::TiledImage::new(1024, 1024, img.format()).expect("image");
    }
    assert_eq!(MaterialMaps::tiles_reached(&big, &o, &wide, &NO_FILL_CONTEXT, &["paint"], &[(3, 3)]), Err(WholeReason::TooFar));
    assert_eq!(MaterialMaps::tiles_reached(&set, &o, &wide, &NO_FILL_CONTEXT, &["nope"], &[(0, 0)]), Err(WholeReason::ReadsEverything));
    let mut small =
        crate::paint::TiledImage::new(64, 64, crate::paint::PixelFormat::new(crate::paint::PixelLayout::Rgba, crate::paint::SampleType::U8)).expect("image");
    small.fill([0.5, 0.5, 1.0, 1.0]);
    set.baked.push(crate::paint::BakedMap { kind: BakedKind::Normal, image: small });
    assert_eq!(MaterialMaps::tiles_reached(&set, &o, &wide, &NO_FILL_CONTEXT, &["paint"], &[(0, 0)]), Err(WholeReason::ResampledBaked));
}
