//! Filters: registration, island separation (no bleed between touching islands), tile independence, and what
//! the single filters do.

use super::*;
use crate::mesh::{Mesh, MeshPart, SurfaceKey, SurfaceMap};
use crate::paint::effect::{EvalRequest, EvalTarget, FilterInput};
use crate::paint::image::Rect;
use crate::paint::{Channel, TextureSet};

const N: u32 = 64;

/// A quad of the mesh at `x` (object units) with UVs `u0..u1` across, 0..1 down.
fn quad(name: &str, x: f32, u0: f32, u1: f32) -> MeshPart {
    MeshPart {
        name: name.into(),
        material: 0,
        positions: vec![[x, -1.0, 0.0], [x + 1.0, -1.0, 0.0], [x + 1.0, 1.0, 0.0], [x, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 4],
        uv_sets: vec![vec![[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]]],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        faces: Default::default(),
    }
}

/// Two islands far apart on the mesh whose UVs touch along u = 0.5.
fn touching() -> SurfaceMap {
    let mesh = Mesh { materials: vec!["m".into()], parts: vec![quad("a", 0.0, 0.0, 0.5), quad("b", 5.0, 0.5, 1.0)] };
    SurfaceMap::build(&mesh, SurfaceKey::square(&mesh, 0, N)).unwrap()
}

fn params(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Light on the left island, dark on the right.
fn halves() -> Vec<[f32; 4]> {
    (0..N * N).map(|i| if i % N < N / 2 { [0.8, 0.8, 0.8, 1.0] } else { [0.2, 0.2, 0.2, 1.0] }).collect()
}

/// Runs filter `id` over `rect` of an N x N set, reading `pixels` (the whole texture).
fn apply(id: &str, p: &[(String, String)], target: EvalTarget, surface: Option<&SurfaceMap>, pixels: &[[f32; 4]], rect: Rect) -> Vec<[f32; 4]> {
    let set = TextureSet::new("s", "S", (N, N)).with_channel(Channel::Roughness);
    let kernel = filter(id).unwrap();
    let request = EvalRequest { set: &set, target, rect, params: p, stack: None, surface };
    let r = kernel.radius(&request);
    let x0 = rect.x.saturating_sub(r);
    let y0 = rect.y.saturating_sub(r);
    let outer = Rect { x: x0, y: y0, w: (rect.x + rect.w + r).min(N) - x0, h: (rect.y + rect.h + r).min(N) - y0 };
    let input: Vec<[f32; 4]> = (0..outer.area()).map(|k| pixels[((outer.y + k as u32 / outer.w) * N + outer.x + k as u32 % outer.w) as usize]).collect();
    let mut out = vec![[0.0; 4]; rect.area()];
    kernel.apply(&request, &FilterInput { rect: outer, pixels: &input }, &mut out).unwrap();
    out
}

fn whole() -> Rect {
    Rect { x: 0, y: 0, w: N, h: N }
}

/// Neighbourhood filters with settings that reach well across the border between the islands.
fn strong() -> Vec<(&'static str, Vec<(String, String)>)> {
    vec![
        (BLUR, params(&[("radius", "0.05")])),
        (SHARPEN, params(&[("radius", "0.02"), ("amount", "3")])),
        (BEVEL, params(&[("distance", "-0.1"), ("smoothing", "0.01")])),
        (MASK_OUTLINE, params(&[("width", "0.1"), ("blur", "0.01")])),
        (WARP, params(&[("intensity", "0.1")])),
        (SLOPE_BLUR, params(&[("intensity", "0.1"), ("mode", "2")])),
    ]
}

#[test]
fn every_filter_is_registered_once() {
    let r = crate::generators::builtin();
    for info in FILTERS {
        assert!(r.filter(info.id).is_some(), "{}", info.id);
        assert_eq!(FILTERS.iter().filter(|f| f.id == info.id).count(), 1);
    }
    assert!(filter("nope").is_none());
}

#[test]
fn nothing_bleeds_between_touching_islands() {
    let surface = touching();
    assert_eq!(surface.island_count(), 2);
    let input = halves();
    for (id, p) in strong() {
        let out = apply(id, &p, EvalTarget::Mask, Some(&surface), &input, whole());
        // each island keeps its own constant value (a mask outline of a uniform island is empty, a bevel
        // of a light island is full)
        let expect = |i: &[f32; 4]| match id {
            MASK_OUTLINE => 0.0,
            BEVEL => (i[0] >= 0.5) as u8 as f32,
            _ => i[0],
        };
        let bleed = out.iter().zip(&input).map(|(o, i)| (o[0] - expect(i)).abs()).fold(0.0f32, f32::max);
        assert!(bleed < 1e-4, "{id}: {bleed} bleeds across the island border");
        // Without the surface map the same filter does mix the halves: the test measures something.
        let mixed = apply(id, &p, EvalTarget::Mask, None, &input, whole());
        let diff = mixed.iter().zip(&input).map(|(o, i)| (o[0] - expect(i)).abs()).fold(0.0f32, f32::max);
        assert!(diff > 0.05, "{id}: no effect without islands ({diff})");
    }
}

#[test]
fn results_do_not_depend_on_tiles() {
    let input: Vec<[f32; 4]> = (0..N * N)
        .map(|i| {
            let (x, y) = ((i % N) as f32, (i / N) as f32);
            let v = if (x - 20.0).hypot(y - 30.0) < 12.0 { 1.0 } else { 0.1 * (x * 0.3).sin().abs() };
            [v, v, v, 1.0]
        })
        .collect();
    let surface = touching();
    let mut all = strong();
    all.push((HSL, params(&[("lightness", "0.3")])));
    all.push((FINISH_ROUGH, params(&[])));
    for (id, p) in all {
        for target in [EvalTarget::Mask, EvalTarget::Channel(Channel::Roughness)] {
            let full = apply(id, &p, target, Some(&surface), &input, whole());
            for (tx, ty) in [(0, 0), (24, 8), (40, 40)] {
                let rect = Rect { x: tx, y: ty, w: 24, h: 24 };
                let tile = apply(id, &p, target, Some(&surface), &input, rect);
                for (k, t) in tile.iter().enumerate() {
                    let (x, y) = (tx + k as u32 % 24, ty + k as u32 / 24);
                    let f = full[(y * N + x) as usize];
                    assert!((0..4).all(|c| (t[c] - f[c]).abs() < 1e-4), "{id} {target:?} at {x},{y}: {t:?} vs {f:?}");
                }
            }
        }
    }
}

fn one(id: &str, p: &[(&str, &str)], target: EvalTarget, v: [f32; 4]) -> [f32; 4] {
    let input = vec![v; (N * N) as usize];
    apply(id, &params(p), target, None, &input, Rect { x: 3, y: 3, w: 1, h: 1 })[0]
}

#[test]
fn per_texel_filters() {
    let m = EvalTarget::Mask;
    assert_eq!(one(INVERT, &[], m, [0.25, 0.25, 0.25, 1.0])[0], 0.75);
    assert_eq!(one(INVERT, &[("invert", "0")], m, [0.25, 0.25, 0.25, 1.0])[0], 0.25);
    let h = EvalTarget::Channel(Channel::Height);
    assert_eq!(one(INVERT, &[], h, [0.25, 0.25, 0.25, 1.0])[0], -0.25);
    assert_eq!(one(HEIGHT_ADJUST, &[("multiplier", "2"), ("offset", "0.5")], h, [1.0, 1.0, 1.0, 1.0])[0], 2.5);
    assert_eq!(one(HEIGHT_ADJUST, &[("invert", "1")], h, [0.3, 0.3, 0.3, 1.0])[0], -0.3);
    assert_eq!(one(CLAMP, &[("min", "0.2"), ("max", "0.6")], m, [0.9, 0.9, 0.9, 1.0])[0], 0.6);
    assert!((one(HISTOGRAM_SCAN, &[], m, [0.3, 0.3, 0.3, 1.0])[0] - 0.3).abs() < 1e-6);
    assert_eq!(one(HISTOGRAM_SCAN, &[("position", "0.5"), ("contrast", "1")], m, [0.6, 0.6, 0.6, 1.0])[0], 1.0);
    assert!((one(HISTOGRAM_SHIFT, &[("position", "0.5")], m, [0.75, 0.75, 0.75, 1.0])[0] - 0.25).abs() < 1e-6);
    assert!((one(CONTRAST_LUMINOSITY, &[("luminosity", "0.1")], m, [0.5, 0.5, 0.5, 1.0])[0] - 0.6).abs() < 1e-6);
    // coverage is kept
    assert_eq!(one(INVERT, &[], m, [0.25, 0.25, 0.25, 0.4])[3], 0.4);
}

#[test]
fn finish_rough_changes_roughness_only() {
    let v = [0.5, 0.5, 0.5, 1.0];
    let input = vec![v; (N * N) as usize];
    let rough = apply(FINISH_ROUGH, &params(&[("intensity", "0.5")]), EvalTarget::Channel(Channel::Roughness), None, &input, whole());
    assert!(rough.iter().any(|p| (p[0] - 0.5).abs() > 0.05));
    let mask = apply(FINISH_ROUGH, &params(&[]), EvalTarget::Mask, None, &input, whole());
    assert!(mask.iter().all(|p| *p == v));
}

#[test]
fn bevel_ramps_inwards_and_outline_follows_the_edge() {
    // a white disc in the middle of a single island
    let input: Vec<[f32; 4]> = (0..N * N)
        .map(|i| {
            let (x, y) = ((i % N) as f32 + 0.5, (i / N) as f32 + 0.5);
            if (x - 32.0).hypot(y - 32.0) < 20.0 { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }
        })
        .collect();
    let bevel = apply(BEVEL, &params(&[("distance", "0.15")]), EvalTarget::Mask, None, &input, whole());
    let at = |px: &[[f32; 4]], x: u32| px[(32 * N + x) as usize][0];
    assert!(at(&bevel, 32) > 0.95 && at(&bevel, 14) < 0.35 && at(&bevel, 5) == 0.0, "{} {} {}", at(&bevel, 32), at(&bevel, 14), at(&bevel, 5));
    let outline = apply(MASK_OUTLINE, &params(&[("width", "0.06")]), EvalTarget::Mask, None, &input, whole());
    assert!(at(&outline, 12) > 0.9 && at(&outline, 32) == 0.0 && at(&outline, 2) == 0.0);
}

#[test]
fn the_compositor_runs_mask_filters_per_island() {
    use crate::paint::effect::Procedural;
    use crate::paint::stack::{Compositor, FillContext, NoFillSources};
    use crate::paint::{BlendMode, EffectKind, Fill, GreyBlend, Layer, LayerKind, Mask, MaskEffect, MaskPixels, PixelFormat, TiledImage};
    let mut paint = TiledImage::new(N, N, PixelFormat::from_id("graya16").unwrap()).unwrap();
    for (i, v) in halves().iter().enumerate() {
        paint.set(i as u32 % N, i as u32 / N, *v);
    }
    let blur = Procedural::new(BLUR).with_params(params(&[("radius", "0.05")]));
    let grey = |kind| MaskEffect::new(GreyBlend { mode: BlendMode::Normal, opacity: 1.0 }, kind);
    let mask = Mask::new(0.0).with_effect(grey(EffectKind::Paint(MaskPixels::new(paint)))).with_effect(grey(EffectKind::Filter(blur)));
    let mut set = TextureSet::new("s", "S", (N, N)).with_channel(Channel::Roughness);
    let mut layer =
        Layer::new("l", "L", LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [1.0; 4])]))).with_blend(Channel::Roughness, BlendMode::Normal, 1.0);
    layer.mask = Some(mask);
    set.layers.push(layer);
    let surface = touching();
    let fills = FillContext::with_sources(&NoFillSources).with_surface(&surface, None);
    let (tex, warnings) = Compositor::with_effects(&set, crate::generators::builtin()).with_fills(&fills).composite_tex(Channel::Roughness);
    assert!(warnings.is_empty(), "{warnings:?}");
    // white over the default 0.5 roughness, by the mask
    let expected: Vec<f32> = halves().iter().map(|m| 0.5 + 0.5 * m[0]).collect();
    let bleed = tex.px.iter().zip(&expected).map(|(o, e)| (o[0] - e).abs()).fold(0.0f32, f32::max);
    assert!(bleed < 3e-3, "{bleed}");
    let (mixed, _) = Compositor::with_effects(&set, crate::generators::builtin()).composite_tex(Channel::Roughness);
    assert!(mixed.px.iter().zip(&expected).any(|(o, e)| (o[0] - e).abs() > 0.05));
}
