//! Tile-by-tile maps against the whole build: the same bits, also after painting into a few tiles.

use super::*;
use crate::paint::effect::Procedural;
use crate::paint::height_normal::HeightFilter;
use crate::paint::stack::NO_FILL_CONTEXT;
use crate::paint::test_stacks::{local_stacks, paint_disc};
use crate::paint::{ContentEffect, EffectKind, LayerKind, PaintContent};
use crate::tex::Tex;

fn options(filter: HeightFilter) -> OutputOptions {
    OutputOptions { height_filter: filter, ..OutputOptions::default() }
}

fn full(set: &TextureSet, o: &OutputOptions) -> MaterialMaps {
    MaterialMaps::build_with(set, o, &crate::paint::effect::registry::EMPTY_REGISTRY, &NO_FILL_CONTEXT)
}

fn tiles(set: &TextureSet, o: &OutputOptions, t: &[(u32, u32)]) -> Vec<TileMaps> {
    MaterialMaps::build_tiles(set, o, &crate::paint::effect::registry::EMPTY_REGISTRY, &NO_FILL_CONTEXT, t).expect("local stack")
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
        assert!(diff.is_none(), "{what}: {c:?} differs at texel {diff:?}");
    }
}

#[test]
fn every_tile_matches_the_whole_build() {
    for filter in [HeightFilter::Sobel, HeightFilter::Backward] {
        for (k, set) in local_stacks().iter().enumerate() {
            let o = options(filter);
            let whole = full(set, &o);
            let blank = MaterialMaps { warnings: Vec::new(), ..whole.clone() };
            let rebuilt = patched(&blank, &tiles(set, &o, &all_tiles(set)));
            assert_same(&rebuilt, &MaterialMaps { warnings: Vec::new(), ..whole }, &format!("stack {k}, {filter:?}"));
        }
    }
}

#[test]
fn painting_a_few_tiles_then_updating_them_matches_a_full_recompute() {
    for (k, set) in local_stacks().iter().enumerate() {
        let o = options(HeightFilter::Sobel);
        let before = full(set, &o);
        let mut painted = set.clone();
        let Some(LayerKind::Paint(p)) = painted.layer_mut("paint").map(|l| &mut l.kind) else { panic!("stack {k} has no paint layer") };
        let mut changed = Vec::new();
        for (_, img) in p.images.iter_mut() {
            img.clear_dirty();
            // across a tile corner, so four tiles change
            paint_disc(img, (128, 128), 20, 99 + k as u32);
            changed.extend(img.dirty_tiles());
        }
        let reached = MaterialMaps::tiles_reached(&painted, &o, &changed);
        assert!(reached.len() > changed.len().min(4), "the normal reaches the neighbours");
        let updated = patched(&before, &tiles(&painted, &o, &reached));
        assert_same(&updated, &full(&painted, &o), &format!("stack {k}"));
    }
}

#[test]
fn non_local_stacks_and_resampled_baked_maps_are_refused() {
    let mut set = local_stacks().remove(0);
    let o = OutputOptions::default();
    let mut blurred = set.clone();
    let mut l = crate::paint::Layer::new("b", "B", LayerKind::Paint(PaintContent::default()));
    l.effects.push(ContentEffect::new(Vec::new(), EffectKind::Filter(Procedural::new("blur"))));
    blurred.layers.push(l);
    let none = &crate::paint::effect::registry::EMPTY_REGISTRY;
    assert!(MaterialMaps::build_tiles(&blurred, &o, none, &NO_FILL_CONTEXT, &[(0, 0)]).is_none());
    let mut small =
        crate::paint::TiledImage::new(64, 64, crate::paint::PixelFormat::new(crate::paint::PixelLayout::Rgba, crate::paint::SampleType::U8)).expect("image");
    small.fill([0.5, 0.5, 1.0, 1.0]);
    set.baked.push(crate::paint::BakedMap { kind: BakedKind::Normal, image: small });
    assert!(MaterialMaps::build_tiles(&set, &o, none, &NO_FILL_CONTEXT, &[(0, 0)]).is_none());
}

#[test]
fn tiles_reached_adds_the_ring_only_for_derived_normals() {
    let set = local_stacks().remove(0);
    let o = OutputOptions::default();
    assert_eq!(MaterialMaps::tiles_reached(&set, &o, &[(0, 0)]), vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
    let flat = OutputOptions { height_normal_strength: 0.0, ..o };
    assert_eq!(MaterialMaps::tiles_reached(&set, &flat, &[(0, 0), (9, 9)]), vec![(0, 0)]);
}
