use super::*;
use crate::history::PaintHistory;
use crate::paint::edit::ops::{instantiate, paste_instance};
use crate::paint::edit::{Edit, EditError, LayerProperty, LayerRef, LayerSlot, StackEffect, Transaction};
use crate::paint::effect::{ContentEffect, EffectKind};
use crate::paint::validate::{InstanceProblem, instance_issues, missing_channels};
use crate::paint::{BlendMode, Channel, Compositor, Document, Fill, Layer, LayerKind, PaintContent, PixelFormat, Rect, TextureSet, TiledImage};

fn fill(id: &str, v: f32) -> Layer {
    Layer::new(id, id, LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [v; 4])]))).with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
}

fn folder(id: &str, children: Vec<Layer>) -> Layer {
    Layer::new(id, id, LayerKind::Group(children)).with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
}

fn instance(id: &str, set: &str, layer: &str) -> Layer {
    Layer::new(id, id, LayerKind::Instance(InstanceLink::new(set, layer)))
}

fn set(id: &str) -> TextureSet {
    TextureSet::new(id, id, (8, 8)).with_channel(Channel::Roughness)
}

/// Set "a" holds a fill "src" and a folder "f" with a fill inside; sets "b" and "c" are empty.
fn doc() -> Document {
    let mut a = set("a");
    a.layers = vec![fill("src", 0.3), folder("f", vec![fill("inner", 0.6)])];
    Document { texture_sets: vec![a, set("b"), set("c")], ..Document::default() }
}

/// The roughness of texel (0, 0) of set `i` as composited, instances resolved.
fn roughness(d: &Document, i: usize) -> f32 {
    let set = d.resolved_set(i).unwrap();
    let mut px = [[0.0f32; 4]; 1];
    Compositor::new(&set).composite_rect(Channel::Roughness, Rect { x: 0, y: 0, w: 1, h: 1 }, &mut px);
    px[0][0]
}

fn apply(d: &mut Document, op: impl FnOnce(&mut Transaction) -> Result<(), EditError>) -> Result<(), EditError> {
    let mut tx = Transaction::new(d);
    op(&mut tx)?;
    tx.commit();
    Ok(())
}

#[test]
fn an_instance_composites_as_its_source_and_follows_its_edits() {
    let mut d = doc();
    d.texture_sets[1].layers.push(instance("i", "a", "src"));
    assert!((roughness(&d, 1) - 0.3).abs() < 1e-3);
    let src = LayerRef::new("a", "src");
    apply(&mut d, |tx| tx.apply(Edit::SetLayer { layer: src.clone(), property: LayerProperty::Fill(Fill::uniform(vec![(Channel::Roughness, [0.8; 4])])) }))
        .unwrap();
    assert!((roughness(&d, 1) - 0.8).abs() < 1e-3);
    // Own name and visibility; the rest from the source.
    let resolved = d.resolved_set(1).unwrap();
    let l = &resolved.layers[0];
    assert_eq!((l.id.as_str(), l.name.as_str()), ("i", "i"));
    assert_eq!(l.blend, d.texture_sets[0].layers[0].blend);
    d.texture_sets[1].layers[0].visible = false;
    assert!((roughness(&d, 1) - Channel::Roughness.default_value()[0]).abs() < 1e-3);
    // A set without instances is borrowed as it is.
    assert!(matches!(d.resolved_set(2), Some(std::borrow::Cow::Borrowed(_))));
}

#[test]
fn painted_pixels_stay_in_the_source_set() {
    let mut d = doc();
    let mut img = TiledImage::new(8, 8, PixelFormat::from_id("graya8").unwrap()).unwrap();
    img.fill([1.0; 4]);
    let mut paint = PaintContent::default();
    paint.set_image(Channel::Roughness, img.clone());
    let painted = Layer::new("p", "P", LayerKind::Paint(paint))
        .with_blend(Channel::Roughness, BlendMode::Normal, 1.0)
        .with_effect(ContentEffect::new(Vec::new(), EffectKind::Fill(Fill::uniform(vec![]))))
        .with_mask(img);
    d.texture_sets[0].layers = vec![painted];
    d.texture_sets[1].layers.push(instance("i", "a", "p"));
    assert!((roughness(&d, 0) - 1.0).abs() < 0.01);
    let resolved = d.resolved_set(1).unwrap();
    let LayerKind::Paint(p) = &resolved.layers[0].kind else { panic!("not a paint layer") };
    assert!(p.images.is_empty());
    let mask = resolved.layers[0].mask.as_ref().unwrap();
    assert!(mask.images().all(|(_, i)| i.allocated_tiles() == 0 && i.size() == (8, 8)));
    assert_eq!(resolved.layers[0].effects.len(), 1, "effects are parameters, they come along");
    assert!((roughness(&d, 1) - Channel::Roughness.default_value()[0]).abs() < 1e-3);
}

#[test]
fn a_folder_instance_mirrors_its_source() {
    let mut d = doc();
    d.texture_sets[1].layers.push(instance("i", "a", "f"));
    assert!((roughness(&d, 1) - 0.6).abs() < 1e-3);
    apply(&mut d, |tx| tx.apply(Edit::InsertLayer { set: "a".into(), at: LayerSlot::in_group("f", 1), layer: Box::new(fill("added", 0.1)) })).unwrap();
    assert!((roughness(&d, 1) - 0.1).abs() < 1e-3, "a layer added to the source folder appears in the instance");
    let resolved = d.resolved_set(1).unwrap();
    let ids: Vec<_> = resolved.walk_layers().map(|(_, l)| l.id.clone()).collect();
    assert_eq!(ids, ["i", "i>inner", "i>added"]);
}

#[test]
fn instances_are_read_only_but_keep_their_name_and_visibility() {
    let mut d = doc();
    d.texture_sets[1].layers.push(instance("i", "b", "x"));
    let r = LayerRef::new("b", "i");
    let refused = |d: &mut Document, e: Edit| {
        let before = d.clone();
        let err = e.apply(d).unwrap_err();
        assert_eq!(*d, before);
        err
    };
    assert!(matches!(refused(&mut d, Edit::SetLayer { layer: r.clone(), property: LayerProperty::Opacity(0.5) }), EditError::InstanceReadOnly { .. }));
    let effect = StackEffect::Content(ContentEffect::new(Vec::new(), EffectKind::Fill(Fill::uniform(vec![]))));
    assert!(matches!(refused(&mut d, Edit::InsertEffect { layer: r.clone(), index: 0, effect }), EditError::InstanceReadOnly { .. }));
    Edit::SetLayer { layer: r.clone(), property: LayerProperty::Name("mine".into()) }.apply(&mut d).unwrap();
    Edit::SetLayer { layer: r, property: LayerProperty::Visible(false) }.apply(&mut d).unwrap();
}

#[test]
fn instancing_refuses_cycles() {
    let mut d = doc();
    let f = LayerRef::new("a", "f");
    // An instance of a folder inside that folder.
    let before = d.clone();
    let e = apply(&mut d, |tx| paste_instance(tx, &f, "a", Some("inner")).map(|_| ())).unwrap_err();
    assert!(matches!(e, EditError::InstanceCycle { .. }), "{e}");
    assert_eq!(d, before);
    // Moving an instance into the folder it depends on.
    apply(&mut d, |tx| paste_instance(tx, &f, "a", None).map(|_| ())).unwrap();
    let before = d.clone();
    let e = Edit::MoveLayer { layer: LayerRef::new("a", "instance1"), to: LayerSlot::in_group("f", 0) }.apply(&mut d).unwrap_err();
    assert!(matches!(e, EditError::InstanceCycle { .. }));
    assert_eq!(d, before);
    // Through another set: "g" in b holds an instance of f; an instance of g inside f closes the loop.
    d.texture_sets[1].layers.push(folder("g", vec![instance("of-f", "a", "f")]));
    let e = apply(&mut d, |tx| paste_instance(tx, &LayerRef::new("b", "g"), "a", Some("inner")).map(|_| ())).unwrap_err();
    assert!(matches!(e, EditError::InstanceCycle { .. }));
    // IN-12: an instance inside a source folder is fine when it refers to another source.
    apply(&mut d, |tx| paste_instance(tx, &LayerRef::new("a", "src"), "a", Some("inner")).map(|_| ())).unwrap();
    assert!(!d.instance_on_cycle("a", "instance2"));
}

#[test]
fn an_existing_cycle_is_broken_not_repaired() {
    let mut d = doc();
    d.texture_sets[0].layers[1] = folder("f", vec![fill("inner", 0.6), instance("loop", "a", "f")]);
    d.texture_sets[1].layers.push(instance("i", "a", "f"));
    d.texture_sets[2].layers.push(instance("gone", "a", "nothing"));
    assert!(d.instance_on_cycle("a", "loop"));
    let index = InstanceIndex::of(&d);
    assert_eq!(index.role("a", "loop"), InstanceRole::Broken);
    assert_eq!(index.role("c", "gone"), InstanceRole::Broken);
    assert_eq!(index.role("a", "f"), InstanceRole::Source);
    assert_eq!(index.role("a", "inner"), InstanceRole::Normal);
    // "i" itself is not on the cycle: it resolves, with the looping instance inside left out.
    assert_eq!(index.role("b", "i"), InstanceRole::Instance);
    assert!((roughness(&d, 1) - 0.6).abs() < 1e-3);
    let resolved = d.resolved_set(0).unwrap();
    assert!(matches!(resolved.layer("loop").map(|l| &l.kind), Some(LayerKind::Instance(_))), "left unresolved");
    let problems: Vec<_> = instance_issues(&d).into_iter().map(|i| (i.layer, i.problem)).collect();
    assert_eq!(problems, [("loop".to_string(), InstanceProblem::Cycle), ("gone".to_string(), InstanceProblem::MissingSource)]);
    assert!(d.validate().is_ok(), "a broken instance does not make the document invalid");
    // The cycle stays until it is undone by hand: moving the instance out of the folder fixes it.
    Edit::MoveLayer { layer: LayerRef::new("a", "loop"), to: LayerSlot::top(0) }.apply(&mut d).unwrap();
    assert!(!d.instance_on_cycle("a", "loop"));
}

#[test]
fn instantiate_across_sets_with_history() {
    let mut d = doc();
    d.texture_sets[2] = TextureSet::new("c", "c", (8, 8)).with_channel(Channel::BaseColor);
    let src = LayerRef::new("a", "src");
    assert_eq!(missing_channels(&d, &InstanceLink::new("a", "src"), &d.texture_sets[2]), [Channel::Roughness]);
    assert!(missing_channels(&d, &InstanceLink::new("a", "src"), &d.texture_sets[1]).is_empty());
    let before = d.clone();
    let mut tx = Transaction::new(&mut d);
    let made = instantiate(&mut tx, &src, &["a".into(), "b".into(), "c".into()]).unwrap();
    let inverses = tx.commit();
    let mut history = PaintHistory::default();
    history.record_all(&d, "Instantiate", None, inverses);
    assert_eq!(made, [LayerRef::new("a", "instance1"), LayerRef::new("b", "instance1"), LayerRef::new("c", "instance1")]);
    let ids: Vec<_> = d.texture_sets[0].layers.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["src", "instance1", "f"], "next to the source in its own set");
    let index = InstanceIndex::of(&d);
    assert_eq!(index.instances_of("a", "src").count(), 3);
    assert_eq!(index.role("a", "src"), InstanceRole::Source);
    let issues = instance_issues(&d);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].problem, InstanceProblem::MissingChannels(vec![Channel::Roughness]));
    d.validate().unwrap();
    // An instance of an instance links to the source.
    let mut tx = Transaction::new(&mut d);
    paste_instance(&mut tx, &LayerRef::new("b", "instance1"), "b", None).unwrap();
    assert_eq!(tx.doc().texture_sets[1].layer("instance2").and_then(|l| l.instance_of()), Some(&InstanceLink::new("a", "src")));
    tx.rollback();
    assert!(history.undo(&mut d).unwrap());
    assert_eq!(d, before);
}

#[test]
fn dependents_are_the_sets_reading_the_changed_layer() {
    let mut d = doc();
    d.texture_sets[1].layers.push(instance("i", "a", "f"));
    d.texture_sets[2].layers.push(folder("g", vec![instance("j", "b", "i")]));
    assert_eq!(d.instance_dependents(&[("a", "inner")]), [1, 2], "through the folder, and through an instance of the instance");
    assert_eq!(d.instance_dependents(&[("a", "f")]), [1, 2]);
    assert!(d.instance_dependents(&[("a", "src")]).is_empty());
    assert!(d.instance_dependents(&[]).is_empty());
    assert_eq!(d.instance_dependents(&[("b", "i")]), [2]);
    assert_eq!(d.instance_source(&InstanceLink::new("b", "i")), Some(InstanceLink::new("a", "f")));
}
