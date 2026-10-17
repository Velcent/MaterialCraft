//! Painter's effects map one to one onto our base content and effects; computable ones composite as before.

use super::fixtures::*;
use crate::model::LayerKind as PainterKind;
use crate::model::*;
use materialcraft::paint::{BlendMode, Channel, Document, EffectKind, LayerKind, Mask, TextureSet};

fn kinds(effects: &[materialcraft::paint::ContentEffect]) -> Vec<&str> {
    effects.iter().map(|e| e.kind.id()).collect()
}

fn filter(name: &str) -> PainterEffect {
    let source = procedural(0, name);
    let PainterSource::Procedural(p) = source else {
        unreachable!()
    };
    PainterEffect::Filter(FilterEffect {
        header: header(ROUGH, vec![]),
        procedural: p.procedural,
        inputs: vec![],
    })
}

fn group(effects: Vec<PainterEffect>) -> PainterEffect {
    PainterEffect::Group(EffectGroup {
        header: header(ALL, vec![blend(ALL, 25, 1.0)]),
        effects,
    })
}

fn anchor(uid: u64) -> PainterEffect {
    let mut h = header(0, vec![]);
    h.uid = Some(uid);
    h.label = "Anchor".into();
    PainterEffect::Anchor(h)
}

fn reference_fill(anchor_uid: u64) -> PainterEffect {
    let reference = PainterSource::Reference(ReferenceSource {
        uid: None,
        channels: ChannelMask(0),
        anchor_uid: Some(anchor_uid),
        anchor_label: String::new(),
        levels: None,
        bindings: vec![],
        mode: 2,
    });
    PainterEffect::Fill(FillEffect {
        header: header(0, vec![]),
        sources: vec![reference],
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    })
}

fn levels() -> PainterEffect {
    PainterEffect::Levels(LevelsEffect {
        header: header(ROUGH, vec![]),
        levels: None,
    })
}

#[test]
fn a_fill_layer_keeps_its_effects_in_order() {
    let content = vec![
        fill(vec![uniform(ROUGH, [0.3; 3]), procedural(BASE, "moss")]),
        filter("sharpen"),
        levels(),
        anchor(9),
    ];
    let stack = import(vec![layer(1, "Moss", content, vec![])]);
    let l = &stack.layers[0];
    let LayerKind::Fill(f) = &l.kind else {
        panic!("fill base")
    };
    assert_eq!(f.uniform_value(Channel::Roughness), Some([0.3, 0.3, 0.3, 1.0]));
    assert_eq!(kinds(&l.effects), ["generator", "filter", "levels", "anchor"]);
    assert_eq!(
        l.effects[0].blend.iter().map(|b| b.channel).collect::<Vec<_>>(),
        [Channel::BaseColor]
    );
    assert!(matches!(&l.effects[0].kind, EffectKind::Generator(p) if p.id == "substance:moss"));
    assert_eq!(
        l.effects[1].blend.iter().map(|b| b.channel).collect::<Vec<_>>(),
        [Channel::Roughness]
    );
    assert!(matches!(&l.effects[3].kind, EffectKind::Anchor(a) if a.id == "anchor-9"));
    assert_eq!(l.effects[3].label, "Anchor");
    assert!(
        !stack.verdict.channel(Channel::Roughness).is_computable(),
        "the filter is not computable"
    );
}

#[test]
fn several_fills_composite_as_painter_does() {
    let half = PainterEffect::Fill(FillEffect {
        header: header(ALL, vec![blend(ROUGH, 4, 0.5)]),
        sources: vec![uniform(ROUGH, [0.5; 3])],
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    });
    let stack = import(vec![layer(
        1,
        "Two fills",
        vec![fill(vec![uniform(ROUGH, [0.8; 3])]), half],
        vec![],
    )]);
    let l = &stack.layers[0];
    assert_eq!(kinds(&l.effects), ["fill"]);
    let b = l.effects[0].blend_for(Channel::Roughness).unwrap();
    assert_eq!((b.mode, b.opacity), (BlendMode::Multiply, 0.5));
    assert!(stack.verdict.channel(Channel::Roughness).is_exact());
    // 0.8 × (1 − 0.5) + 0.8 × 0.5 × 0.5
    assert!((composite(&stack, Channel::Roughness, 0, 0)[0] - 0.6).abs() < 1e-3);
}

#[test]
fn a_fill_below_full_strength_stays_an_effect() {
    let weak = PainterEffect::Fill(FillEffect {
        header: header(ALL, vec![blend(ROUGH, 2, 0.25)]),
        sources: vec![uniform(ROUGH, [1.0; 3])],
        projection: 0,
        filtering: 2,
        placement: Default::default(),
        uv_transform: None,
    });
    let base = layer(1, "Base", vec![fill(vec![uniform(ROUGH, [0.0; 3])])], vec![]);
    let stack = import(vec![base, layer(2, "Weak", vec![weak], vec![])]);
    let l = &stack.layers[1];
    assert!(
        matches!(&l.kind, LayerKind::Fill(f) if f.channels().is_empty()),
        "empty base"
    );
    assert_eq!(kinds(&l.effects), ["fill"]);
    assert_eq!(l.blend_for(Channel::Roughness).map(|b| b.opacity), Some(1.0));
    assert!((composite(&stack, Channel::Roughness, 0, 0)[0] - 0.25).abs() < 1e-3);
}

#[test]
fn a_paint_layer_holds_its_brush_work_and_effects() {
    let strokes = PainterEffect::Paint(PaintEffect {
        header: header(ALL, vec![]),
        tool: PaintTool::Brush,
        opacity: 1.0,
        strokes: vec![],
        path_strokes: 3,
        brush_uid: None,
        colors: vec![],
        alpha: vec![],
        stencil: None,
    });
    let decal = PainterEffect::PolygonFill(PolygonFillEffect {
        header: header(ALL, vec![]),
        hits: vec![],
        unread: 2,
    });
    let mut paint = layer(
        1,
        "Paint",
        vec![
            group(vec![strokes.clone(), decal]),
            fill(vec![uniform(ROUGH, [0.2; 3])]),
            group(vec![strokes]),
        ],
        vec![],
    );
    paint.kind = PainterKind::Paint;
    let stack = import(vec![paint]);
    let l = &stack.layers[0];
    assert!(matches!(&l.kind, LayerKind::Paint(p) if p.images.is_empty()));
    assert_eq!(kinds(&l.effects), ["polygon_fill", "fill", "paint"]);
    assert_eq!(
        l.effects[2].blend_for(Channel::Roughness).map(|b| b.mode),
        Some(BlendMode::Passthrough)
    );
    assert!(!stack.verdict.channel(Channel::BaseColor).is_computable());
}

#[test]
fn masks_map_effect_for_effect_and_references_find_their_anchor() {
    let mut l = layer(
        2,
        "Masked",
        vec![fill(vec![uniform(ROUGH, [1.0; 3])]), anchor(5)],
        vec![],
    );
    l.mask = Some(PainterMask {
        enabled: true,
        initial: MaskInitial::Black,
        effects: vec![group(vec![]), reference_fill(5), filter("blur"), levels(), anchor(6)],
    });
    let stack = import(vec![l]);
    let m = stack.layers[0].mask.as_ref().unwrap();
    let ids: Vec<&str> = m.effects.iter().map(|e| e.kind.id()).collect();
    assert_eq!(ids, ["paint", "reference", "filter", "levels", "anchor"]);
    assert!(matches!(&m.effects[1].kind, EffectKind::Reference(r) if r.anchor == "anchor-5"));
    assert_eq!(
        stack.verdict.channel(Channel::Roughness).reasons(),
        ["mask filter \"blur\""],
        "the reference is computable; first reason only"
    );
}

#[test]
fn imported_stacks_are_valid_documents() {
    let mut l = layer(1, "A", vec![fill(vec![uniform(ROUGH, [1.0; 3])]), anchor(5)], vec![]);
    l.mask = Some(PainterMask {
        enabled: true,
        initial: MaskInitial::White,
        effects: vec![anchor(5), group(vec![])],
    });
    let stack = import(vec![l.clone(), l]);
    let mut set = TextureSet::new("t", "T", SIZE);
    set.channels = channels().specs().to_vec();
    set.layers = stack.layers;
    let doc = Document {
        texture_sets: vec![set],
        ..Document::default()
    };
    assert_eq!(doc.validate(), Ok(()), "anchor ids stay unique");
    let mut bytes = Vec::new();
    materialcraft::paint::format::save(&doc, &mut bytes).unwrap();
    let back = materialcraft::paint::format::load(std::io::Cursor::new(bytes)).unwrap();
    assert_eq!(back, doc);
    assert!(
        back.texture_sets[0].layers[0]
            .mask
            .as_ref()
            .is_some_and(Mask::is_identity)
    );
}
