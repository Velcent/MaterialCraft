//! Texture fills: procedural textures of the effect registry laid on by the fill's projection, per channel.

use super::*;
use crate::paint::effect::{EvalRequest, GeneratorSource, Params, Procedural, Reach};
use crate::paint::{ContentEffect, EffectKind, Fill, FillSource, TextureFill};
use crate::projection::{Filter, ImagePlacement, Projection, Tap};

const R: Channel = Channel::Roughness;

/// A texture whose value is the image coordinate's u (to see the projection reach it), times the `gain` parameter.
struct URamp;

impl GeneratorSource for URamp {
    fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
        None
    }
    fn generate(&self, _request: &EvalRequest, _out: &mut [[f32; 4]]) -> Result<(), String> {
        Err("points only".into())
    }

    fn samples_points(&self) -> bool {
        true
    }

    fn sample_points(&self, params: &[(String, String)], taps: &[Tap], out: &mut [[f32; 4]]) -> Result<(), String> {
        let gain = Params::new(params).f("gain", 1.0);
        for (t, o) in taps.iter().zip(out) {
            let v = t.uv[0].rem_euclid(1.0) * gain;
            *o = [v, v, v, 1.0];
        }
        Ok(())
    }
}

/// A generator that only works on rectangles.
struct RectOnly;

impl GeneratorSource for RectOnly {
    fn stack_reach(&self, _: &EvalRequest) -> Option<Reach> {
        None
    }
    fn generate(&self, _request: &EvalRequest, out: &mut [[f32; 4]]) -> Result<(), String> {
        out.fill([1.0; 4]);
        Ok(())
    }
}

fn registry() -> EffectRegistry {
    let mut r = EffectRegistry::new();
    r.add_generator("u_ramp", Box::new(URamp));
    r.add_generator("rect_only", Box::new(RectOnly));
    r
}

fn texture_fill(id: &str, projection: Projection) -> Fill {
    let mut f = TextureFill::default();
    f.set_texture(R, Procedural::new(id));
    f.set_value(Channel::BaseColor, [0.2, 0.4, 0.6, 1.0]);
    Fill::new(FillSource::Texture(f)).with_projection(Projection { filter: Filter::Nearest, ..projection })
}

fn set_with(fill: Fill) -> TextureSet {
    let mut s = TextureSet::new("s", "S", (4, 4)).with_channel(R).with_channel(Channel::BaseColor);
    s.layers.push(Layer::new("base", "Base", LayerKind::Fill(Fill::uniform(vec![(R, [0.9; 4])]))).with_blend(R, BlendMode::Normal, 1.0));
    let layer =
        Layer::new("tex", "Texture", LayerKind::Fill(fill)).with_blend(R, BlendMode::Normal, 1.0).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0);
    s.layers.push(layer);
    s
}

fn row(s: &TextureSet, c: Compositor) -> Vec<f32> {
    let (tex, _) = c.composite_tex(R);
    (0..s.resolution.0 as usize).map(|x| tex.px[x][0]).collect()
}

fn close(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 2e-3)
}

#[test]
fn a_texture_channel_is_sampled_through_the_fill_projection() {
    let effects = registry();
    let s = set_with(texture_fill("u_ramp", Projection::default()));
    let c = Compositor::with_effects(&s, &effects);
    assert!(close(&row(&s, c), &[0.125, 0.375, 0.625, 0.875]), "{:?}", row(&s, c));
    assert!(c.warnings(R).is_empty());
    // twice the tiling: the texture repeats twice across the set (UV placement scales about the centre)
    let tiled = Projection::uv(ImagePlacement { tiling: [2.0, 2.0], ..ImagePlacement::default() });
    let s = set_with(texture_fill("u_ramp", tiled));
    let c = Compositor::with_effects(&s, &effects);
    assert!(close(&row(&s, c), &[0.75, 0.25, 0.75, 0.25]), "{:?}", row(&s, c));
    // the constant channel is the value, whatever the projection
    let (tex, _) = c.composite_tex(Channel::BaseColor);
    assert!((tex.px[5][2] - 0.6).abs() < 1e-3);
}

#[test]
fn parameters_reach_the_texture() {
    let effects = registry();
    let mut fill = texture_fill("u_ramp", Projection::default());
    if let FillSource::Texture(t) = &mut fill.source
        && let Some(p) = t.texture_mut(R)
    {
        p.params.push(("gain".into(), "0.5".into()));
    }
    let s = set_with(fill);
    assert!(close(&row(&s, Compositor::with_effects(&s, &effects)), &[0.0625, 0.1875, 0.3125, 0.4375]));
}

#[test]
fn unknown_textures_are_left_out_and_reported() {
    let effects = registry();
    for id in ["nothing", "rect_only"] {
        let s = set_with(texture_fill(id, Projection::default()));
        let c = Compositor::with_effects(&s, &effects);
        assert!(close(&row(&s, c), &[0.9; 4]), "{id}");
        assert_eq!(c.warnings(R), vec![CompositeWarning::TextureFillSkipped { layer: "tex".into(), texture: id.into() }]);
        // the constant channel still composites, without a warning
        assert!(c.warnings(Channel::BaseColor).is_empty());
    }
    // the same in a content effect
    let mut s = set_with(Fill::uniform(vec![(R, [0.3; 4])]));
    let fx = ContentEffect::new(Vec::new(), EffectKind::Fill(texture_fill("nothing", Projection::default()))).with_blend(R, BlendMode::Normal, 1.0);
    s.layers[1].effects.push(fx);
    let c = Compositor::with_effects(&s, &effects);
    assert!(close(&row(&s, c), &[0.3; 4]));
    assert_eq!(c.warnings(R), vec![CompositeWarning::EffectNotComputable { layer: "tex".into(), in_mask: false, index: 0, kind: "fill".into() }]);
}

#[test]
fn a_texture_fill_effect_blends_over_the_base() {
    let effects = registry();
    let mut s = set_with(Fill::uniform(vec![(R, [0.0; 4])]));
    let fx = ContentEffect::new(Vec::new(), EffectKind::Fill(texture_fill("u_ramp", Projection::default()))).with_blend(R, BlendMode::Normal, 0.5);
    s.layers[1].effects.push(fx);
    assert!(close(&row(&s, Compositor::with_effects(&s, &effects)), &[0.0625, 0.1875, 0.3125, 0.4375]));
}

#[test]
fn the_built_in_textures_fill_channels() {
    let mut f = TextureFill::default();
    f.set_texture(R, Procedural::new("checker"));
    let s = set_with(Fill::new(FillSource::Texture(f)));
    let c = Compositor::with_effects(&s, crate::generators::builtin());
    assert!(c.warnings(R).is_empty());
    let values = row(&s, c);
    assert!(values.iter().all(|v| (0.0..=1.0).contains(v)));
    assert_ne!(values, vec![0.9; 4], "the texture replaced the base");
}
