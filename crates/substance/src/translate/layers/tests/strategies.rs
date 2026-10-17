//! The per-set strategy: arranging Painter's result, defaults, cache disagreement and hidden masks.

use super::fixtures::*;
use crate::translate::layers::cache_agreement::CacheAgreement;
use crate::translate::layers::cache_score::Score;
use crate::translate::layers::strategy::provides_base;
use crate::translate::layers::{PAINTER_RESULT_NAME, REFERENCE_NAME, StackParts, StackStrategy};
use materialcraft::paint::{Channel, Fill, Layer, LayerKind};

fn plain(id: &str) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![])))
}

#[test]
fn strategies_arrange_the_painter_result_and_defaults() {
    let parts = |complete| StackParts {
        imported: vec![plain("a")],
        painter_result: Some(plain("result")),
        defaults: plain("defaults"),
        result_complete: complete,
        hide_unless_covered: vec![],
    };
    let ids = |layers: &[Layer]| layers.iter().map(|l| l.id.clone()).collect::<Vec<_>>();
    let real = StackStrategy::RealStack.assemble(parts(true), &channels());
    assert_eq!(ids(&real), ["defaults", "a", "result"]);
    assert_eq!((real[2].name.as_str(), real[2].visible), (REFERENCE_NAME, false));
    let top = StackStrategy::PainterResultOnTop.assemble(parts(false), &channels());
    assert_eq!(ids(&top), ["a", "defaults", "result"]);
    assert_eq!((top[2].name.as_str(), top[2].visible), (PAINTER_RESULT_NAME, true));
    assert_eq!(
        ids(&StackStrategy::PainterResultOnTop.assemble(parts(true), &channels())),
        ["a", "result"]
    );
    assert_eq!(
        ids(&StackStrategy::BestEffort.assemble(parts(true), &channels())),
        ["defaults", "a"]
    );
}

#[test]
fn a_full_uniform_base_replaces_the_defaults() {
    let all = vec![
        uniform(BASE, [0.5; 3]),
        uniform(HEIGHT, [0.0; 3]),
        uniform(ROUGH, [0.5; 3]),
    ];
    let stack = import(vec![layer(1, "Base", vec![fill(all)], vec![blend(HEIGHT, 2, 1.0)])]);
    assert!(provides_base(&stack.layers, &channels()));
    let parts = StackParts {
        imported: stack.layers,
        painter_result: None,
        defaults: plain("defaults"),
        result_complete: false,
        hide_unless_covered: vec![],
    };
    let layers = StackStrategy::RealStack.assemble(parts, &channels());
    assert_eq!(layers.len(), 1);
}

#[test]
fn a_disagreeing_cache_keeps_painter_result_on_top() {
    let stack = import(vec![layer(
        1,
        "Base",
        vec![fill(vec![uniform(BASE, [0.5; 3])])],
        vec![],
    )]);
    let stale = CacheAgreement {
        disagreeing: vec![(
            Channel::BaseColor,
            Score {
                mean: 147.0,
                max: 188.0,
            },
        )],
    };
    let strategy = StackStrategy::choose(&stack.verdict, true, &stale);
    assert_eq!(strategy, StackStrategy::CacheDisagrees);
    assert!(strategy.shows_painter_result());
    let parts = StackParts {
        imported: stack.layers,
        painter_result: Some(plain("result")),
        defaults: plain("defaults"),
        result_complete: true,
        hide_unless_covered: vec![],
    };
    let layers = strategy.assemble(parts, &channels());
    assert_eq!(
        (layers[1].name.as_str(), layers[1].visible),
        (PAINTER_RESULT_NAME, true)
    );
}

#[test]
fn layers_with_uncomputable_masks_are_hidden_unless_painter_result_covers_them() {
    let mut masked = layer(2, "Dirt", vec![fill(vec![uniform(BASE, [0.1; 3])])], vec![]);
    masked.mask = Some(crate::model::PainterMask {
        enabled: true,
        initial: crate::model::MaskInitial::Black,
        effects: vec![fill(vec![procedural(0, "mg_dirt")])],
    });
    let inner = folder(3, "Folder", vec![masked], vec![]);
    let stack = import(vec![
        layer(1, "Base", vec![fill(vec![uniform(BASE, [0.5; 3])])], vec![]),
        inner,
    ]);
    assert_eq!(stack.verdict.mask_failures.len(), 1);
    assert_eq!(stack.verdict.mask_failures[0].path, "Folder / Dirt");
    let parts = |layers: Vec<Layer>| StackParts {
        hide_unless_covered: stack.verdict.mask_failures.iter().map(|f| f.id.clone()).collect(),
        imported: layers,
        painter_result: Some(plain("result")),
        defaults: plain("defaults"),
        result_complete: true,
    };
    let best = StackStrategy::BestEffort.assemble(parts(stack.layers.clone()), &channels());
    assert!(!best[2].children()[0].visible, "hidden without Painter's result on top");
    let top = StackStrategy::PainterResultOnTop.assemble(parts(stack.layers.clone()), &channels());
    assert!(
        top[1].children()[0].visible,
        "Painter's visibility kept under its result"
    );
}
