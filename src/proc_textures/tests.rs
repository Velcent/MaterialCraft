use super::*;
use crate::paint::TextureSet;
use crate::paint::effect::{EvalRequest, EvalTarget, GeneratorSource};
use crate::paint::image::Rect;

fn none() -> Vec<(String, String)> {
    Vec::new()
}

fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// Mean absolute jump of the texture across the vertical line at `u` (just left of it against just right of it).
fn jump(t: &ProcTexture, u: f32) -> f32 {
    let d = 1e-5;
    (0..64).map(|y| (t.value(u - d, y as f32 / 64.0) - t.value(u + d, y as f32 / 64.0)).abs()).sum::<f32>() / 64.0
}

#[test]
fn ids_are_unique_and_every_texture_builds() {
    for (i, a) in TEXTURES.iter().enumerate() {
        assert!(TEXTURES.iter().skip(i + 1).all(|b| b.id != a.id), "duplicate id {}", a.id);
        assert!(texture(a.id, &none()).is_some());
    }
    assert!(texture("nope", &none()).is_none());
    // textures and mask generators share one registry: an id may name only one of them
    for g in crate::generators::GENERATORS {
        assert!(info(g.id).is_none(), "{} is a generator and a texture", g.id);
    }
}

#[test]
fn values_stay_in_range_vary_and_repeat_without_a_seam() {
    let n = 128;
    for info in TEXTURES {
        let t = texture(info.id, &kv(&[("seed", "3")])).unwrap_or_else(|| ProcTexture::plain(Box::new(Flat)));
        let mut values = Vec::new();
        for y in 0..n {
            for x in 0..n {
                let v = t.value(x as f32 / n as f32, y as f32 / n as f32);
                assert!((0.0..=1.0).contains(&v), "{} out of range: {v}", info.id);
                values.push(v);
            }
        }
        let (lo, hi) = values.iter().fold((1.0f32, 0.0f32), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        assert!(hi - lo > 0.05, "{} is flat", info.id);
        // the jump across the tile border is no larger than the largest jump across grid-aligned lines inside
        let inner = (1..16).map(|k| jump(&t, k as f32 / 16.0)).fold(0.0f32, f32::max);
        let seam = jump(&t, 1.0);
        assert!(seam <= inner + 0.02, "{} has a seam: {seam} vs {inner}", info.id);
    }
}

struct Flat;

impl Look for Flat {
    fn value(&self, _u: f32, _v: f32) -> f32 {
        0.5
    }
}

#[test]
fn seeds_and_parameters_change_the_look() {
    let a = texture("clouds_2", &kv(&[("seed", "1")])).map(|t| t.value(0.3, 0.7));
    let b = texture("clouds_2", &kv(&[("seed", "2")])).map(|t| t.value(0.3, 0.7));
    assert_ne!(a, b);
    let plain = texture("perlin", &none()).map(|t| t.value(0.3, 0.7));
    let inverted = texture("perlin", &kv(&[("invert", "1")])).map(|t| t.value(0.3, 0.7));
    assert_eq!(plain.map(|v| 1.0 - v), inverted);
}

#[test]
fn tone_identity_and_steps() {
    let t = Tone::default();
    assert!(t.is_identity());
    assert_eq!(t.apply(0.37), 0.37);
    let hard = Tone { contrast: 1.0, ..Tone::default() };
    assert_eq!((hard.apply(0.4), hard.apply(0.6)), (0.0, 1.0));
    let bright = Tone { balance: 0.8, ..Tone::default() };
    assert!(bright.apply(0.5) > 0.5);
}

#[test]
fn generator_writes_the_rectangle_through_the_placement() {
    let set = TextureSet::new("s", "S", (16, 16));
    let mut registry = EffectRegistry::new();
    register(&mut registry);
    let source = registry.generator("cells_1").map(|_| ());
    assert!(source.is_some());
    let params = kv(&[("seed", "4")]);
    let rect = Rect { x: 4, y: 2, w: 8, h: 4 };
    let request = EvalRequest { set: &set, target: EvalTarget::Mask, rect, params: &params, stack: None, surface: None };
    let mut out = vec![[0.0f32; 4]; rect.area()];
    TextureSource::new("cells_1").generate(&request, &mut out).unwrap_or_else(|e| panic!("{e}"));
    let t = texture("cells_1", &params).map(|t| t.value(4.5 / 16.0, 2.5 / 16.0));
    assert_eq!(Some(out[0][0]), t);
    assert!(out.iter().all(|p| p[3] == 1.0));
    // tiling 2 through the UV placement: the texture repeats twice across the set
    let tiled = kv(&[("seed", "4"), ("tiling", "2")]);
    assert_ne!(TextureSource::projection(&tiled), crate::projection::Projection::default());
    let request = EvalRequest { params: &tiled, rect: Rect { x: 0, y: 0, w: 16, h: 16 }, ..request };
    let mut full = vec![[0.0f32; 4]; 256];
    TextureSource::new("cells_1").generate(&request, &mut full).unwrap_or_else(|e| panic!("{e}"));
    for y in 0..16 {
        for x in 0..8 {
            assert!((full[y * 16 + x][0] - full[y * 16 + x + 8][0]).abs() < 1e-4);
        }
    }
}
