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
    // An empty (or unknown) selection is refused with a reason instead of doing nothing.
    assert!(matches!(refused(&mut d, |tx| duplicate(tx, "s", &sel(&["nope"]))), EditError::NotAccepted(_)));
    assert!(matches!(refused(&mut d, |tx| duplicate_to(tx, "s", &[], LayerSlot::top(0))), EditError::NotAccepted(_)));
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
    step(&mut d, |tx| add_mask(tx, &lr("a"), MaskStart::Generator("heights")));
    let generated = d.texture_sets[0].layer("a").unwrap().mask.as_ref().unwrap();
    assert!(matches!(&generated.effects[0].kind, EffectKind::Generator(p) if p.id == "heights"));
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

fn smart_mask(effects: Vec<MaskEffect>) -> DropPayload {
    DropPayload::SmartMask { effects, asset: None, embed: Vec::new() }
}

#[test]
fn a_smart_mask_records_where_it_came_from_and_embeds_itself() {
    use crate::paint::embedded::EmbeddedAsset;
    let mut d = doc();
    let effects = vec![MaskEffect::new(GreyBlend::default(), EffectKind::Fill(1.0))];
    let reference = "smart-masks/edges.mcfrag";
    let smart = DropPayload::SmartMask { effects, asset: Some(reference.into()), embed: vec![EmbeddedAsset::new(reference, vec![1, 2, 3])] };
    let asset = |d: &Document, id: &str| d.texture_sets[0].layer(id).unwrap().mask.as_ref().unwrap().asset.clone();
    step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoLayer(lr("base")), &smart, false));
    assert_eq!(asset(&d, "base").as_deref(), Some(reference), "a new mask");
    assert_eq!(d.assets.iter().map(|a| a.reference.as_str()).collect::<Vec<_>>(), [reference]);
    // Added on top of a mask with its own effects: the mask is not the smart mask's any more than before.
    step(&mut d, |tx| add_mask(tx, &lr("b"), MaskStart::ColorSelection));
    step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoMask(lr("b")), &smart, false));
    assert_eq!(asset(&d, "b"), None, "appended to a mask with effects");
    step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoMask(lr("b")), &smart, true));
    assert_eq!(asset(&d, "b").as_deref(), Some(reference), "replaced");
    assert_eq!(d.assets.len(), 1, "embedded once");
}

#[test]
fn a_smart_mask_makes_a_black_mask_if_needed() {
    let mut d = doc();
    let smart = smart_mask(vec![MaskEffect::new(GreyBlend::default(), EffectKind::Fill(1.0)), MaskEffect::new(GreyBlend::default(), EffectKind::Fill(0.5))]);
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
    assert!(matches!(refused(&mut d, |tx| drop_onto(tx, &DropTarget::OntoLayer(lr("a")), &smart_mask(Vec::new()), false)), EditError::NotAccepted(_)));
}

#[test]
fn a_smart_mask_gets_fresh_anchor_ids_and_the_sets_resolution() {
    use crate::paint::effect::{MaskPixels, Reference};
    use crate::paint::{PixelFormat, TiledImage};
    let mut d = doc();
    let mut pixels = TiledImage::new(16, 16, PixelFormat::from_id("graya8").unwrap()).unwrap();
    pixels.fill([1.0; 4]);
    let smart = smart_mask(vec![
        MaskEffect::new(GreyBlend::default(), EffectKind::Paint(MaskPixels::new(pixels))),
        MaskEffect::new(GreyBlend::default(), EffectKind::Anchor(Anchor { id: "pin1".into() })),
        MaskEffect::new(GreyBlend::default(), EffectKind::Reference(Reference::new("pin1"))),
    ]);
    // Twice: the second drop must not collide with the first, nor with the set's own "pin1".
    for target in ["base", "b"] {
        step(&mut d, |tx| drop_onto(tx, &DropTarget::OntoLayer(lr(target)), &smart, false));
    }
    let effects = |id: &str| d.texture_sets[0].layer(id).unwrap().mask.clone().unwrap().effects;
    let anchor_and_reference = |id: &str| match &effects(id)[1..] {
        [MaskEffect { kind: EffectKind::Anchor(a), .. }, MaskEffect { kind: EffectKind::Reference(r), .. }] => (a.id.clone(), r.anchor.clone()),
        other => panic!("{other:?}"),
    };
    assert_eq!(anchor_and_reference("base"), ("pin2".into(), "pin2".into()));
    assert_eq!(anchor_and_reference("b"), ("pin3".into(), "pin3".into()));
    assert!(matches!(&effects("b")[0].kind, EffectKind::Paint(p) if p.image.size() == (64, 64)));
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

#[test]
fn a_fill_gets_a_texture_source_and_keeps_its_projection() {
    use crate::paint::edit::{Edit, StackEffect};
    use crate::paint::{FillSource, TextureFill};
    let mut d = doc();
    let r = FillRef::new(lr("f"), FillAt::Layer);
    let projection = crate::projection::Projection::uv(crate::projection::ImagePlacement::default());
    step(&mut d, |tx| set_projection(tx, &r, projection.clone()));
    let mut textures = TextureFill::default();
    textures.set_texture(Channel::Roughness, Procedural::new("perlin"));
    let source = FillSource::Texture(textures);
    step(&mut d, |tx| set_fill_source(tx, &r, source.clone()));
    assert_eq!(r.fill(&d).map(|f| (&f.source, &f.projection)), Some((&source, &projection)));
    // a fill effect too; other layers and effects refuse
    let fx = ContentEffect::new(Vec::new(), EffectKind::Fill(Fill::uniform(vec![])));
    step(&mut d, |tx| tx.apply(Edit::InsertEffect { layer: lr("base"), index: 0, effect: StackEffect::Content(fx) }));
    let e = FillRef::new(lr("base"), FillAt::Effect(0));
    step(&mut d, |tx| set_fill_source(tx, &e, source.clone()));
    assert_eq!(e.fill(&d).map(|f| &f.source), Some(&source));
    refused(&mut d, |tx| set_fill_source(tx, &FillRef::new(lr("base"), FillAt::Layer), source.clone()));
    refused(&mut d, |tx| set_fill_source(tx, &FillRef::new(lr("a"), FillAt::Effect(0)), source.clone()));
}
