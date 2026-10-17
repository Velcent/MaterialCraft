//! Layers with every effect kind in their content and mask stacks, used by the full test document.

use super::fixtures::noisy;
use crate::json;
use crate::paint::effect::{Anchor, ColorSelection, Levels, Pick, PickMode, PolygonFill, Procedural, Reference, Selection, UnknownEffect};
use crate::paint::{
    BakedKind, BlendMode, Channel, ChannelSpec, ContentEffect, EffectKind, Fill, FillSource, GeometryMask, GreyBlend, Layer, LayerKind, Mask, MaskEffect,
    MaskPixels, PaintContent, PixelFormat,
};

fn procedural(id: &str) -> Procedural {
    let original = Procedural::new(format!("substance:{id}")).with_params(vec![("level".into(), "0.5,1".into())]);
    Procedural::new(id).with_params(vec![("amount".into(), "0.25".into()), ("seed".into(), "7".into())]).replacing(original)
}

fn grey(mode: BlendMode, opacity: f32, kind: EffectKind<crate::paint::effect::Grey>) -> MaskEffect {
    MaskEffect::new(GreyBlend { mode, opacity }, kind)
}

/// A fill layer whose content and mask hold every effect kind (and an unknown one), with a geometry mask.
pub fn effects_layer(w: u32, h: u32) -> Layer {
    let base = Fill::uniform(vec![(Channel::Roughness, [0.4, 0.4, 0.4, 1.0])]);
    let mut paint = PaintContent::default();
    paint.set_image(Channel::BaseColor, noisy(w, h, PixelFormat::from_id("rgba8").unwrap(), 31));
    paint.set_image(Channel::Height, noisy(w, h, ChannelSpec::new(Channel::Height).layer_format(), 32));
    let rough = |kind| ContentEffect::new(Vec::new(), kind).with_blend(Channel::Roughness, BlendMode::Multiply, 0.5);
    let selection = Selection { mode: PickMode::UvIsland, picks: vec![Pick { part: "body".into(), triangles: vec![0, 1] }, Pick::default()] };
    let unknown = json::parse(r#"{"kind": "warp3d", "enabled": false, "strength": [1, 2], "nested": {"a": "b"}}"#).unwrap();
    let mut disabled = rough(EffectKind::Levels(Levels { gamma: [0.5, 1.0, 2.0], clamp: false, ..Levels::default() }));
    disabled.enabled = false;
    let mut anchor = rough(EffectKind::Anchor(Anchor { id: "anchor-1".into() }));
    anchor.label = "Rust anchor".into();
    let effects = vec![
        ContentEffect::new(Vec::new(), EffectKind::Paint(paint)).with_blend(Channel::BaseColor, BlendMode::Overlay, 0.75).with_blend(
            Channel::Height,
            BlendMode::SignedAdd,
            1.0,
        ),
        rough(EffectKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.9, 0.9, 0.9, 1.0])]))),
        rough(EffectKind::Fill(Fill::new(FillSource::Material { path: "materials/rust.mca".into(), params: vec![("Amount".into(), "1".into())] }))),
        rough(EffectKind::Generator(procedural("dirt"))),
        rough(EffectKind::Filter(procedural("blur"))),
        disabled,
        rough(EffectKind::ColorSelection(ColorSelection { map: BakedKind::Id, colors: vec![[1.0, 0.0, 0.5], [0.1, 0.2, 0.3]], tolerance: 0.1, hardness: 0.9 })),
        rough(EffectKind::PolygonFill(PolygonFill { selection: selection.clone(), values: Fill::uniform(vec![(Channel::Roughness, [0.1, 0.1, 0.1, 1.0])]) })),
        anchor,
        rough(EffectKind::Reference(Reference { anchor: "anchor-1".into() })),
        ContentEffect::new(Vec::new(), EffectKind::Unknown(UnknownEffect { kind: "warp3d".into(), data: unknown.clone() })),
    ];
    let mut mask = Mask::new(1.0);
    mask.effects = vec![
        grey(BlendMode::Normal, 1.0, EffectKind::Paint(MaskPixels::new(noisy(w, h, PixelFormat::from_id("graya16").unwrap(), 33)))),
        grey(BlendMode::Multiply, 0.5, EffectKind::Fill(0.3)),
        grey(BlendMode::Subtract, 1.0, EffectKind::Generator(procedural("edge_wear"))),
        grey(BlendMode::Normal, 1.0, EffectKind::Filter(procedural("sharpen"))),
        grey(BlendMode::Normal, 1.0, EffectKind::Levels(Levels::default())),
        grey(
            BlendMode::Add,
            0.25,
            EffectKind::ColorSelection(ColorSelection { map: BakedKind::Other("id2".into()), colors: vec![], tolerance: 0.0, hardness: 1.0 }),
        ),
        grey(BlendMode::Normal, 1.0, EffectKind::PolygonFill(PolygonFill { selection, values: 1.0 })),
        grey(BlendMode::Normal, 1.0, EffectKind::Anchor(Anchor { id: "mask-anchor".into() })),
        grey(BlendMode::Lighten, 1.0, EffectKind::Reference(Reference { anchor: "anchor-1".into() })),
        grey(BlendMode::Normal, 1.0, EffectKind::Unknown(UnknownEffect { kind: "warp3d".into(), data: unknown })),
    ];
    mask.enabled = false;
    let mut layer = Layer::new("effects", "Effects", LayerKind::Fill(base)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0);
    layer.set_blend(Channel::Roughness, BlendMode::Normal, 0.5);
    layer.set_blend(Channel::Height, BlendMode::SignedAdd, 1.0);
    layer.effects = effects;
    layer.mask = Some(mask);
    layer.geometry_mask = Some(GeometryMask::Parts(vec!["body".into(), "trim 2".into()]));
    layer
}

/// A group with effects, a half-grey initial mask and a UV-tile geometry mask.
pub fn effects_group() -> Layer {
    let mut g = Layer::new("fx-group", "FX group", LayerKind::Group(vec![])).with_blend(Channel::Roughness, BlendMode::Normal, 1.0);
    g.effects.push(ContentEffect::new(Vec::new(), EffectKind::Filter(Procedural::new("hsl"))).with_blend(Channel::Roughness, BlendMode::Normal, 1.0));
    g.mask = Some(Mask::new(0.5));
    g.geometry_mask = Some(GeometryMask::UvTiles(vec![1001, 1002]));
    g
}
