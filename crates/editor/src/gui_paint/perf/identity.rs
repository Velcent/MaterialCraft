//! Incremental results against full recomputes on real documents (`MC_PERF_DOCS`), bit for bit: the maps built
//! with the mask cache after edits of a few layers, and, for sets that can be composited tile by tile, the tiles
//! after painting into a paint layer. Ignored; run locally like the benchmarks (`perf_identity`).

use materialcraft::generators;
use materialcraft::paint::stack::StackCache;
use materialcraft::paint::{Channel, LayerKind, MaterialMaps, NO_FILL_CONTEXT, OutputOptions, TextureSet};
use std::path::Path;

#[test]
#[ignore]
fn perf_identity() {
    let list = std::env::var("MC_PERF_DOCS").unwrap_or_default();
    for path in list.split(':').filter(|p| !p.is_empty()).map(Path::new).filter(|p| p.exists()) {
        let doc = materialcraft::paint::format::load_path(path).expect("load");
        for (i, set) in doc.texture_sets.iter().enumerate() {
            let set = doc.resolved_set(i).map_or_else(|| set.clone(), |r| r.into_owned());
            let edits = cached_after_edits(&set);
            let tiles = tiles_after_paint(&set);
            eprintln!("{} / {}: cache identical after {edits} edits; tiles: {tiles}", path.display(), set.name);
        }
    }
}

fn same(a: &MaterialMaps, b: &MaterialMaps) -> bool {
    Channel::STANDARD.iter().all(|&c| match (a.map(c), b.map(c)) {
        (Some(x), Some(y)) => x.px.iter().zip(&y.px).all(|(p, q)| p.map(f32::to_bits) == q.map(f32::to_bits)),
        _ => true,
    })
}

/// Builds with a cache, edits (opacity, visibility of top-level layers spread over the stack), builds again with
/// the cache and fresh; returns the number of edits checked.
fn cached_after_edits(set: &TextureSet) -> usize {
    let o = OutputOptions::default();
    let reg = generators::builtin();
    let cache = StackCache::default();
    let _ = MaterialMaps::build_cached(set, &o, reg, &NO_FILL_CONTEXT, Some(&cache.session(set, 1)));
    let mut s = set.clone();
    let n = s.layers.len();
    let mut checked = 0;
    for k in [n / 4, n / 2, n.saturating_sub(1)] {
        let Some(l) = s.layers.get_mut(k) else { continue };
        l.opacity *= 0.5;
        if k == n / 2 {
            l.visible = !l.visible;
        }
        let cached = MaterialMaps::build_cached(&s, &o, reg, &NO_FILL_CONTEXT, Some(&cache.session(&s, 1)));
        let fresh = MaterialMaps::build_with(&s, &o, reg, &NO_FILL_CONTEXT);
        assert!(same(&cached, &fresh), "{}: the cached build differs after editing layer {k}", set.name);
        checked += 1;
    }
    checked
}

/// Paints into the first paint layer and compares the tiles it reaches with a full build.
fn tiles_after_paint(set: &TextureSet) -> String {
    let o = OutputOptions::default();
    let reg = generators::builtin();
    if MaterialMaps::build_tiles(set, &o, reg, &NO_FILL_CONTEXT, &[]).is_none() {
        return "not local".into();
    }
    let mut s = set.clone();
    let Some(img) = s.layers.iter_mut().find_map(|l| match &mut l.kind {
        LayerKind::Paint(p) => p.images.first_mut().map(|(_, i)| i),
        _ => None,
    }) else {
        return "no paint layer".into();
    };
    img.clear_dirty();
    let (w, h) = (img.width(), img.height());
    for y in h / 3..h / 3 + 40 {
        for x in w / 3..w / 3 + 300 {
            img.set(x.min(w - 1), y.min(h - 1), [0.9, 0.2, 0.1, 1.0]);
        }
    }
    let changed = img.dirty_tiles();
    let reached = MaterialMaps::tiles_reached(&s, &o, &changed);
    let tiles = MaterialMaps::build_tiles(&s, &o, reg, &NO_FILL_CONTEXT, &reached).expect("local");
    let full = MaterialMaps::build_with(&s, &o, reg, &NO_FILL_CONTEXT);
    for t in &tiles {
        for c in Channel::STANDARD {
            let (Some(src), Some(map)) = (t.map(c), full.map(c)) else { continue };
            for (i, p) in src.iter().enumerate() {
                let (x, y) = (t.rect.x as usize + i % t.rect.w as usize, t.rect.y as usize + i / t.rect.w as usize);
                assert_eq!(p.map(f32::to_bits), map.px[y * map.w + x].map(f32::to_bits), "{}: tile texel ({x}, {y}) of {c:?}", s.name);
            }
        }
    }
    format!("{} tiles identical", tiles.len())
}
