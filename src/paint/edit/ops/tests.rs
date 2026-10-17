//! Every panel operation: applied through a transaction, recorded in a history, undone to the identical
//! document and redone to the identical result; failures leave the document alone.

use super::*;
use crate::history::PaintHistory;
use crate::paint::doc::Document;
use crate::paint::edit::{LayerRef, StackId};
use crate::paint::effect::{Anchor, ContentEffect, EffectKind, GreyBlend, MaskEffect, Procedural};
use crate::paint::layer::{Fill, LayerKind, PaintContent};
use crate::paint::{BlendMode, Channel, Layer, TextureSet};

fn layer(id: &str) -> Layer {
    Layer::new(id, id, LayerKind::Paint(PaintContent::default())).with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
}

fn doc() -> Document {
    let mut set = TextureSet::new("s", "S", (64, 64)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    let anchored = layer("a").with_effect(ContentEffect::new(Vec::new(), EffectKind::Anchor(Anchor { id: "pin1".into() })));
    let group = Layer::new("g", "G", LayerKind::Group(vec![anchored, layer("b")])).with_blend(Channel::Roughness, BlendMode::Normal, 1.0);
    set.layers = vec![layer("base"), group, Layer::new("f", "F", LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.3; 4])])))];
    let mut d = Document::default();
    d.texture_sets.push(set);
    d.validate().unwrap();
    d
}

fn ids(d: &Document) -> Vec<String> {
    d.texture_sets[0].walk_layers().map(|(depth, l)| format!("{depth}{}", l.id)).collect()
}

fn sel(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn lr(id: &str) -> LayerRef {
    LayerRef::new("s", id)
}

/// Runs `op` as one history step; checks it changed the document validly, that undo gives back the identical
/// document and redo the identical result.
fn step<T>(d: &mut Document, op: impl FnOnce(&mut Transaction) -> Result<T, EditError>) -> T {
    let before = d.clone();
    let mut tx = Transaction::new(d);
    let out = op(&mut tx).unwrap_or_else(|e| panic!("{e}"));
    let inverses = tx.commit();
    let mut history = PaintHistory::default();
    history.record_all(d, "op", None, inverses);
    d.validate().unwrap();
    assert_ne!(*d, before, "the operation changed nothing");
    let after = d.clone();
    assert!(history.undo(d).unwrap());
    assert_eq!(*d, before, "undo");
    assert!(history.redo(d).unwrap());
    assert_eq!(*d, after, "redo");
    out
}

/// `op` fails and the document is as before.
fn refused<T: std::fmt::Debug>(d: &mut Document, op: impl FnOnce(&mut Transaction) -> Result<T, EditError>) -> EditError {
    let before = d.clone();
    let mut tx = Transaction::new(d);
    let e = op(&mut tx).expect_err("should fail");
    drop(tx);
    assert_eq!(*d, before);
    e
}

#[test]
fn create_above_the_selection_or_on_top() {
    let mut d = doc();
    let id = step(&mut d, |tx| create(tx, "s", NewLayer::Paint, Some("a")));
    assert_eq!(id, "layer1");
    assert_eq!(ids(&d), ["0base", "0g", "1a", "1layer1", "1b", "0f"]);
    let l = d.texture_sets[0].layer("layer1").unwrap();
    assert_eq!((l.name.as_str(), l.blend.len()), ("Layer 1", 2));
    let LayerKind::Paint(p) = &l.kind else { panic!() };
    assert_eq!(p.images.len(), 2);
    assert_eq!(step(&mut d, |tx| create(tx, "s", NewLayer::Fill, None)), "fill1");
    assert_eq!(step(&mut d, |tx| create(tx, "s", NewLayer::Folder, Some("base"))), "folder1");
    assert_eq!(ids(&d), ["0base", "0folder1", "0g", "1a", "1layer1", "1b", "0f", "0fill1"]);
    assert!(matches!(refused(&mut d, |tx| create(tx, "nope", NewLayer::Paint, None)), EditError::NoTextureSet(_)));
}

#[test]
fn duplicate_copies_with_new_ids_above_each_original() {
    let mut d = doc();
    let copies = step(&mut d, |tx| duplicate(tx, "s", &sel(&["g", "a", "f"])));
    assert_eq!(copies, ["g1", "f1"], "a is inside the selected group");
    assert_eq!(ids(&d), ["0base", "0g", "1a", "1b", "0g1", "1a1", "1b1", "0f", "0f1"]);
    let copy = d.texture_sets[0].layer("a1").unwrap();
    assert!(matches!(&copy.effects[0].kind, EffectKind::Anchor(a) if a.id == "pin2"), "anchors get new ids");
    assert_eq!(d.texture_sets[0].layer("g1").unwrap().name, "G copy");
    let placed = step(&mut d, |tx| duplicate_to(tx, "s", &sel(&["base", "b"]), LayerSlot::top(0)));
    assert_eq!(placed, ["base1", "b2"]);
    assert_eq!(ids(&d)[..2], ["0base1", "0b2"]);
}

#[test]
fn delete_removes_the_selection() {
    let mut d = doc();
    step(&mut d, |tx| delete(tx, "s", &sel(&["b", "f", "unknown"])));
    assert_eq!(ids(&d), ["0base", "0g", "1a"]);
    step(&mut d, |tx| delete(tx, "s", &sel(&["g", "a"])));
    assert_eq!(ids(&d), ["0base"]);
    assert!(matches!(refused(&mut d, |tx| delete(tx, "s", &[])), EditError::NotAccepted(_)));
}

#[test]
fn group_keeps_order_and_takes_the_place_of_the_topmost() {
    let mut d = doc();
    let id = step(&mut d, |tx| group(tx, "s", &sel(&["f", "base", "b"])));
    assert_eq!(id, "folder1");
    assert_eq!(ids(&d), ["0g", "1a", "0folder1", "1base", "1b", "1f"]);
    assert!(matches!(refused(&mut d, |tx| group(tx, "s", &sel(&["nope"]))), EditError::NotAccepted(_)));
}

#[test]
fn move_reorders_into_and_out_of_folders() {
    let mut d = doc();
    // To the top of the stack, keeping order.
    step(&mut d, |tx| move_layers(tx, "s", &sel(&["b", "base"]), LayerSlot::top(3)));
    assert_eq!(ids(&d), ["0g", "1a", "0f", "0base", "0b"]);
    // Into a folder (LS-15), between its children.
    step(&mut d, |tx| move_layers(tx, "s", &sel(&["f", "b"]), LayerSlot::in_group("g", 0)));
    assert_eq!(ids(&d), ["0g", "1f", "1b", "1a", "0base"]);
    // Out again, below everything.
    step(&mut d, |tx| move_layers(tx, "s", &sel(&["a"]), LayerSlot::top(0)));
    assert_eq!(ids(&d), ["0a", "0g", "1f", "1b", "0base"]);
    // A slot counted before the move: just above "g" means index 2 at the top level.
    step(&mut d, |tx| move_layers(tx, "s", &sel(&["a"]), LayerSlot::top(2)));
    assert_eq!(ids(&d), ["0g", "1f", "1b", "0a", "0base"]);
    // Dropping where the layers already are changes nothing and records nothing.
    let mut tx = Transaction::new(&mut d);
    move_layers(&mut tx, "s", &sel(&["a"]), LayerSlot::top(1)).unwrap();
    assert!(tx.is_empty());
    drop(tx);
    assert!(matches!(refused(&mut d, |tx| move_layers(tx, "s", &sel(&["g"]), LayerSlot::in_group("g", 0))), EditError::IntoItself { .. }));
    assert!(matches!(refused(&mut d, |tx| move_layers(tx, "s", &sel(&["a"]), LayerSlot::in_group("base", 0))), EditError::NotAGroup { .. }));
    assert!(matches!(refused(&mut d, |tx| move_layers(tx, "s", &sel(&["a"]), LayerSlot::top(9))), EditError::IndexOutOfRange { .. }));
}

#[test]
fn effects_are_added_to_either_stack_and_switched() {
    let mut d = doc();
    for what in NewEffect::ALL {
        let e = new_effect(&d.texture_sets[0], what, StackId::Content);
        step(&mut d, |tx| add_effect(tx, &lr("base"), e, None));
    }
    let base = d.texture_sets[0].layer("base").unwrap();
    assert_eq!(base.effects.len(), NewEffect::ALL.len());
    assert!(base.effects.iter().all(|e| e.blend.len() == 2));
    step(&mut d, |tx| set_effect_enabled(tx, &lr("base"), StackId::Content, 1, false));
    assert!(!d.texture_sets[0].layer("base").unwrap().effects[1].enabled);
    let mask_effect = new_effect(&d.texture_sets[0], NewEffect::Paint, StackId::Mask);
    assert!(matches!(refused(&mut d, |tx| add_effect(tx, &lr("base"), mask_effect.clone(), None)), EditError::NoMask { .. }));
    step(&mut d, |tx| add_mask(tx, &lr("base"), MaskStart::White));
    for what in NewEffect::ALL {
        let e = new_effect(&d.texture_sets[0], what, StackId::Mask);
        step(&mut d, |tx| add_effect(tx, &lr("base"), e, Some(0)));
    }
    step(&mut d, |tx| set_effect_enabled(tx, &lr("base"), StackId::Mask, 0, false));
    assert!(matches!(refused(&mut d, |tx| set_effect_enabled(tx, &lr("base"), StackId::Mask, 99, false)), EditError::IndexOutOfRange { .. }));
}

#[test]
fn mask_states() {
    let mut d = doc();
    for start in MaskStart::ALL {
        step(&mut d, |tx| add_mask(tx, &lr("a"), start));
    }
    step(&mut d, |tx| invert_mask(tx, &lr("a")));
    assert_eq!(d.texture_sets[0].layer("a").unwrap().mask.as_ref().unwrap().initial, 1.0);
    step(&mut d, |tx| set_mask_enabled(tx, &lr("a"), false));
    step(&mut d, |tx| remove_mask(tx, &lr("a")));
    assert!(matches!(refused(&mut d, |tx| remove_mask(tx, &lr("a"))), EditError::NoMask { .. }));
    assert!(matches!(refused(&mut d, |tx| invert_mask(tx, &lr("a"))), EditError::NoMask { .. }));
}

fn blur() -> DropPayload {
    DropPayload::Effect(EffectAsset { name: "Blur".into(), kind: AssetEffect::Filter(Procedural::new("blur")) })
}

#[test]
fn an_effect_dropped_between_layers_becomes_an_effect_layer() {
    let mut d = doc();
    let target = DropTarget::Between { set: "s".into(), at: LayerSlot::in_group("g", 1) };
    assert!(accepts(&d, &target, &blur()));
    let out = step(&mut d, |tx| drop_onto(tx, &target, &blur(), false));
    assert_eq!(out, DropOutcome::NewLayer("effect1".into()));
    assert_eq!(ids(&d), ["0base", "0g", "1a", "1effect1", "1b", "0f"]);
    let l = d.texture_sets[0].layer("effect1").unwrap();
    assert_eq!(l.name, "Blur");
    assert!(l.blend.iter().all(|b| b.mode == BlendMode::Passthrough) && l.blend.len() == 2);
    assert!(matches!(&l.effects[..], [e] if matches!(&e.kind, EffectKind::Filter(p) if p.id == "blur")));
    assert!(!accepts(&d, &DropTarget::Between { set: "s".into(), at: LayerSlot::in_group("base", 0) }, &blur()));
    assert!(!accepts(&d, &DropTarget::Between { set: "s".into(), at: LayerSlot::top(9) }, &blur()));
}

#[test]
fn an_effect_dropped_onto_a_layer_or_mask_joins_that_stack_and_ctrl_replaces() {
    let mut d = doc();
    let onto = DropTarget::OntoLayer(lr("a"));
    let out = step(&mut d, |tx| drop_onto(tx, &onto, &blur(), false));
    assert_eq!(out, DropOutcome::Effects { layer: lr("a"), stack: StackId::Content, replaced: false });
    let a = d.texture_sets[0].layer("a").unwrap();
    assert_eq!(a.effects.len(), 2, "added on top of the anchor");
    assert_eq!(a.effects[1].label, "Blur");
    step(&mut d, |tx| drop_onto(tx, &onto, &blur(), true));
    assert_eq!(d.texture_sets[0].layer("a").unwrap().effects.len(), 1, "Ctrl replaces");

    let mask = DropTarget::OntoMask(lr("b"));
    assert!(!accepts(&d, &mask, &blur()));
    assert!(matches!(refused(&mut d, |tx| drop_onto(tx, &mask, &blur(), false)), EditError::NoMask { .. }));
    step(&mut d, |tx| add_mask(tx, &lr("b"), MaskStart::ColorSelection));
    assert!(accepts(&d, &mask, &blur()));
    step(&mut d, |tx| drop_onto(tx, &mask, &blur(), false));
    assert_eq!(d.texture_sets[0].layer("b").unwrap().mask.as_ref().unwrap().effects.len(), 2);
    assert!(d.texture_sets[0].layer("b").unwrap().effects.is_empty(), "the content stack is untouched");
    step(&mut d, |tx| drop_onto(tx, &mask, &blur(), true));
    assert_eq!(d.texture_sets[0].layer("b").unwrap().mask.as_ref().unwrap().effects.len(), 1);
}

#[test]
fn a_smart_mask_makes_a_black_mask_if_needed() {
    let mut d = doc();
    let smart = DropPayload::SmartMask(vec![
        MaskEffect::new(GreyBlend::default(), EffectKind::Fill(1.0)),
        MaskEffect::new(GreyBlend::default(), EffectKind::Fill(0.5)),
    ]);
    step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoLayer(lr("base")), &smart, false));
    let m = d.texture_sets[0].layer("base").unwrap().mask.clone().unwrap();
    assert_eq!((m.initial, m.effects.len()), (0.0, 2));
    step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoMask(lr("base")), &smart, false));
    assert_eq!(d.texture_sets[0].layer("base").unwrap().mask.as_ref().unwrap().effects.len(), 4);
    step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoMask(lr("base")), &smart, true));
    assert_eq!(d.texture_sets[0].layer("base").unwrap().mask.as_ref().unwrap().effects.len(), 2);
    let between = DropTarget::Between { set: "s".into(), at: LayerSlot::top(0) };
    assert!(!accepts(&d, &between, &smart));
    assert!(matches!(refused(&mut d, |tx| drop_onto(tx, &between, &smart, false)), EditError::NotAccepted(_)));
    assert!(matches!(
        refused(&mut d, |tx| drop_onto(tx, &DropTarget::OntoLayer(lr("a")), &DropPayload::SmartMask(Vec::new()), false)),
        EditError::NotAccepted(_)
    ));
}

#[test]
fn a_failure_half_way_rolls_back_the_whole_operation() {
    let mut d = doc();
    let before = d.clone();
    {
        let mut tx = Transaction::new(&mut d);
        create(&mut tx, "s", NewLayer::Paint, None).unwrap();
        move_layers(&mut tx, "s", &sel(&["f"]), LayerSlot::top(0)).unwrap();
        assert!(move_layers(&mut tx, "s", &sel(&["g"]), LayerSlot::in_group("a", 0)).is_err());
    }
    assert_eq!(d, before);
}
