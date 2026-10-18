//! The layer panel's instancing (IN-05, IN-07 … IN-10) headless: copy and paste as instance, the instance list's
//! entries, the dialog's rows with their channel warnings, and frames drawn with badges and the dialog open.

use super::*;
use eframe::egui::{self, RawInput, Rect, pos2, vec2};
use materialcraft::paint::instance::InstanceLink;
use materialcraft::paint::{BlendMode, Fill, Layer};
use std::path::PathBuf;

fn fill(id: &str) -> Layer {
    Layer::new(id, format!("Fill {id}"), LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.4; 4])]))).with_blend(
        Channel::Roughness,
        BlendMode::Normal,
        1.0,
    )
}

/// "a" holds the source "f" and a broken instance; "b" an instance of "f"; "c" has no roughness channel.
fn open() -> OpenDocument {
    let mut a = TextureSet::new("a", "Body", (16, 16)).with_channel(Channel::Roughness);
    a.layers = vec![fill("f"), Layer::new("gone", "Gone", LayerKind::Instance(InstanceLink::new("a", "nothing")))];
    let mut b = TextureSet::new("b", "Trim", (16, 16)).with_channel(Channel::Roughness);
    b.layers = vec![fill("base"), Layer::new("inst", "Fill f", LayerKind::Instance(InstanceLink::new("a", "f")))];
    let c = TextureSet::new("c", "Glass", (16, 16)).with_channel(Channel::BaseColor);
    let doc = Document { texture_sets: vec![a, b, c], ..Document::default() };
    OpenDocument::new(doc, PathBuf::from("inst.mcpaint"), None)
}

fn frame(ctx: &egui::Context, panel: &mut LayerPanel, doc: &OpenDocument) -> Vec<PanelEvent> {
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 700.0))), ..RawInput::default() };
    let mut out = Vec::new();
    let mut full = ctx.run_ui(input, |ui| panel.show(ui, doc, &mut out));
    full.textures_delta.clear();
    out
}

#[test]
fn copy_then_paste_as_instance() {
    let doc = open();
    let mut panel = LayerPanel::default();
    assert_eq!(panel.action(doc.doc(), LayerAction::PasteInstance), None, "nothing copied yet");
    panel.select(vec!["f".into()]);
    assert_eq!(panel.action(doc.doc(), LayerAction::Copy), None);
    let pasted = panel.action(doc.doc(), LayerAction::PasteInstance);
    assert_eq!(pasted, Some(LayerEvent::PasteInstance { source: LayerRef::new("a", "f"), set: "a".into(), above: Some("f".into()) }));
}

#[test]
fn the_instance_list_starts_with_the_source() {
    let doc = open();
    let index = InstanceIndex::of(doc.doc());
    let from_instance = instance_list::entries(doc.doc(), &index, &LayerRef::new("b", "inst"));
    let from_source = instance_list::entries(doc.doc(), &index, &LayerRef::new("a", "f"));
    assert_eq!(from_instance, from_source);
    let labels: Vec<_> = from_source.iter().map(|e| (e.label.as_str(), e.source)).collect();
    assert_eq!(labels, [("Body › Fill f", true), ("Trim › Fill f", false)]);
    assert!(instance_list::entries(doc.doc(), &index, &LayerRef::new("b", "base")).is_empty());
    assert!(instance_list::entries(doc.doc(), &index, &LayerRef::new("a", "gone")).is_empty(), "a broken instance has no source to list");
    // Selecting an entry shows its set.
    let mut panel = LayerPanel::default();
    panel.reveal(doc.doc().texture_set("b").unwrap(), "inst");
    assert_eq!(panel.current_set(doc.doc()).map(|s| s.id.as_str()), Some("b"));
    assert_eq!(panel.selection.ids(), ["inst"]);
    assert!(!panel.paintable(doc.doc()), "an instance can't be painted (IN-03)");
}

#[test]
fn the_dialog_marks_sets_without_the_sources_channels() {
    let doc = open();
    let rows = instantiate_dialog::rows(doc.doc(), &LayerRef::new("b", "inst"));
    let summary: Vec<_> = rows.iter().map(|r| (r.id.as_str(), r.own, r.missing.clone())).collect();
    // An instance as the source stands for its own source, set "a".
    assert_eq!(summary, [("a", false, vec![]), ("b", true, vec![]), ("c", false, vec![Channel::Roughness])]);
}

#[test]
fn frames_with_badges_and_the_dialog() {
    let doc = open();
    let ctx = egui::Context::default();
    let mut panel = LayerPanel::default();
    panel.select(vec!["f".into()]);
    frame(&ctx, &mut panel, &doc);
    panel.instantiate = Some(InstantiateDialog::new(LayerRef::new("a", "f")));
    for _ in 0..3 {
        assert!(frame(&ctx, &mut panel, &doc).is_empty());
    }
    assert!(panel.instantiate.is_some(), "the dialog stays open until answered");
    panel.reveal(doc.doc().texture_set("b").unwrap(), "inst");
    frame(&ctx, &mut panel, &doc);
}
