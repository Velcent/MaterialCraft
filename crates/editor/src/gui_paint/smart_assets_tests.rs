//! Step 26's gate: inserting a smart material or mask and undoing it leaves the document byte-identical, and a
//! smart mask that reads ambient occlusion warns on a set without it.

use super::*;
use crate::gui_paint::document_state::OpenDocument;
use crate::gui_paint::panels::layers::LayerEvent;
use crate::gui_paint::stack_editor;
use materialcraft::paint::edit::ops::{DropPayload, DropTarget};
use materialcraft::paint::edit::{LayerRef, LayerSlot};
use materialcraft::paint::effect::{EffectKind, GreyBlend, MaskEffect, Procedural};
use materialcraft::paint::{BakedMap, BlendMode, Channel, EmbeddedAsset, Layer, LayerKind, Mask, PaintContent, PixelFormat, TextureSet, TiledImage, format};
use std::path::PathBuf;

fn open() -> OpenDocument {
    let mut set = TextureSet::new("s", "S", (32, 32)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    set.layers.push(Layer::new("base", "Base", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    OpenDocument::new(doc, PathBuf::from("test.mcpaint"), None)
}

fn bytes(d: &OpenDocument) -> Vec<u8> {
    let mut out = Vec::new();
    format::save(d.doc(), &mut out).unwrap();
    out
}

/// A mask reading ambient occlusion (and curvature) through the dirt generator.
fn dirt_mask() -> Mask {
    Mask::new(0.0).with_effect(MaskEffect::new(GreyBlend::default(), EffectKind::Generator(Procedural::new(generators::DIRT))))
}

fn drop(d: &mut OpenDocument, target: DropTarget, payload: DropPayload) {
    let applied = stack_editor::apply(d, LayerEvent::Drop { target, payload, replace: false }).unwrap();
    assert_eq!(applied.set, Some(0));
}

#[test]
fn inserting_a_smart_material_or_mask_and_undoing_it_leaves_the_document_byte_identical() {
    let mut d = open();
    let before = bytes(&d);
    let layers = vec![Layer::new("p", "Paint", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0)];
    let folder = DropPayload::Folder {
        name: "Worn".into(),
        layers,
        asset: Some("smart-materials/worn.mcfrag".into()),
        embed: vec![EmbeddedAsset::new("smart-materials/worn.mcfrag", vec![1, 2, 3])],
    };
    drop(&mut d, DropTarget::Between { set: "s".into(), at: LayerSlot::top(1) }, folder);
    assert_ne!(bytes(&d), before, "the folder and its embedded asset are in the document");
    d.undo().unwrap();
    assert_eq!(bytes(&d), before, "smart material: undo restores every byte");

    let mask = DropPayload::SmartMask {
        effects: dirt_mask().effects,
        asset: Some("smart-masks/dirt.mcfrag".into()),
        embed: vec![EmbeddedAsset::new("smart-masks/dirt.mcfrag", vec![4, 5])],
    };
    drop(&mut d, DropTarget::OntoLayer(LayerRef::new("s", "base")), mask);
    assert_eq!(d.doc().texture_sets[0].layer("base").unwrap().mask.as_ref().and_then(|m| m.asset.as_deref()), Some("smart-masks/dirt.mcfrag"));
    assert_ne!(bytes(&d), before);
    d.undo().unwrap();
    assert_eq!(bytes(&d), before, "smart mask: undo restores every byte");
}

#[test]
fn a_smart_mask_on_a_set_without_ambient_occlusion_warns() {
    let mut d = open();
    let mut f = make::smart_mask(
        &{
            let mut s = d.doc().texture_sets[0].clone();
            s.layers[0].mask = Some(dirt_mask());
            s
        },
        "base",
    )
    .unwrap();
    f.complete(&generators::baked_maps, |_| None);
    let needs: Vec<String> = f.needs.iter().map(|k| k.id().to_string()).collect();
    assert!(needs.iter().any(|n| n == "ambient_occlusion"), "{needs:?}");

    let missing = MissingMaps::of(d.doc(), "s", &needs).expect("the set has baked nothing");
    assert!(missing.missing.contains(&BakedKind::AmbientOcclusion));
    assert!(missing.bakeable.contains(&MapKind::AmbientOcclusion), "the bakers can make it");
    let warning = missing.warning(&f.name);
    assert!(warning.contains("ambient occlusion") && warning.contains("\"S\""), "{warning}");

    // With every map it needs baked there is nothing to say.
    let maps: Vec<BakedMap> =
        f.needs.iter().map(|k| BakedMap { kind: k.clone(), image: TiledImage::new(32, 32, PixelFormat::from_id("graya8").unwrap()).unwrap() }).collect();
    d.edit("Bake", None, |tx| tx.apply(materialcraft::paint::edit::Edit::SetBaked { set: "s".into(), maps })).unwrap();
    assert_eq!(MissingMaps::of(d.doc(), "s", &needs), None);
    assert_eq!(MissingMaps::of(d.doc(), "nope", &needs), None);
}
