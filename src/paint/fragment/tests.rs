use super::*;
use crate::paint::channel::Channel;
use crate::paint::effect::{ColorSelection, EffectKind, GreyBlend, MaskEffect, MaskPixels, Procedural};
use crate::paint::layer::{Fill, LayerImage, PaintContent};
use crate::paint::{BakedMap, BlendMode, PixelFormat, TiledImage};

fn set() -> TextureSet {
    TextureSet::new("s", "S", (8, 8)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness)
}

fn painted(size: u32, v: f32) -> TiledImage {
    let mut img = TiledImage::new(size, size, PixelFormat::from_id("rgba8").unwrap()).unwrap();
    img.fill([v, v, v, 1.0]);
    img
}

#[test]
fn carrier_round_trip_and_kind() {
    let s = set();
    let layers = vec![Layer::new("a", "A", LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.5; 4])])))];
    let f = Fragment::smart_material("Worn", &s, layers);
    assert_eq!(f.kind(), FragmentKind::SmartMaterial);
    f.validate().unwrap();
    assert_eq!(Fragment::from_carrier(f.kind(), f.carrier(), vec![], vec![]).unwrap(), f);
    let m = Fragment::smart_mask("Edges", &s, Mask::new(0.0));
    assert_eq!(m.kind(), FragmentKind::SmartMask);
    assert!(m.channels.is_empty());
    assert_eq!(Fragment::from_carrier(m.kind(), m.carrier(), vec![], vec![]).unwrap(), m);
    assert!(Fragment::from_carrier(FragmentKind::SmartMask, f.carrier(), vec![], vec![]).is_none());
    for k in [FragmentKind::SmartMaterial, FragmentKind::SmartMask] {
        assert_eq!(FragmentKind::from_id(k.id()), Some(k));
    }
}

#[test]
fn needs_come_from_generators_and_colour_selections() {
    let generator = |id: &str| MaskEffect::new(GreyBlend::default(), EffectKind::Generator(Procedural::new(id)));
    let selection = ColorSelection { map: BakedKind::Id, colors: vec![], tolerance: 0.1, hardness: 0.5 };
    let pick = MaskEffect::new(GreyBlend::default(), EffectKind::ColorSelection(selection));
    let mask = Mask::new(0.0).with_effect(generator("dirt")).with_effect(pick).with_effect(generator("dirt")).with_effect(generator("unknown"));
    let lookup = |id: &str| if id == "dirt" { vec![BakedKind::AmbientOcclusion, BakedKind::Curvature] } else { vec![] };
    let needs = needs::required_maps(&FragmentContent::Mask(mask), &lookup);
    assert_eq!(needs, [BakedKind::AmbientOcclusion, BakedKind::Curvature, BakedKind::Id]);
    let mut s = set();
    s.baked.push(BakedMap { kind: BakedKind::Curvature, image: painted(4, 0.5) });
    assert_eq!(needs::missing(&needs, &s), [BakedKind::AmbientOcclusion, BakedKind::Id]);
}

#[test]
fn fitting_resamples_images_and_drops_undeclared_channels() {
    let mut paint = PaintContent::default();
    paint.images.push((Channel::BaseColor, painted(16, 0.25)));
    paint.images.push((Channel::Emissive, painted(16, 1.0)));
    let mut layer = Layer::new("p", "P", LayerKind::Paint(paint)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0).with_blend(
        Channel::Emissive,
        BlendMode::Normal,
        1.0,
    );
    let mask_paint = MaskEffect::new(GreyBlend::default(), EffectKind::Paint(MaskPixels::new(painted(4, 0.75))));
    layer.mask = Some(Mask::new(0.0).with_effect(mask_paint));
    let mut layers = vec![Layer::new("g", "G", LayerKind::Group(vec![layer]))];
    let mut s = set();
    adapt::fit_layers(&mut layers, &s);
    s.layers = layers;
    let doc = Document { texture_sets: vec![s], ..Document::default() };
    doc.validate().unwrap();
    let l = &doc.texture_sets[0].layers[0].children()[0];
    assert_eq!(l.blend.len(), 1);
    let images = l.images();
    let (_, paint) = images.iter().find(|(k, _)| matches!(k, LayerImage::Paint(_))).unwrap();
    assert_eq!(paint.size(), (8, 8));
    assert!((paint.get(3, 3)[0] - 0.25).abs() < 0.01);
    let (_, mask) = images.iter().find(|(k, _)| matches!(k, LayerImage::Mask { .. })).unwrap();
    assert!((mask.get(7, 7)[0] - 0.75).abs() < 0.01);
}
