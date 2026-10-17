//! The layer panel headless: a 200-layer stack stays interactive (frame times measured with thumbnails
//! being computed off the UI thread), clicks select, and the asset tray's drop-target API finds targets.

use super::*;
use eframe::egui::{self, Event, PointerButton, RawInput, Rect, pos2, vec2};
use materialcraft::paint::edit::ops::{AssetEffect, EffectAsset};
use materialcraft::paint::effect::Procedural;
use materialcraft::paint::{BlendMode, ChannelSpec, Layer, PaintContent, TiledImage};
use std::path::PathBuf;
use std::time::{Duration, Instant};

const SCREEN: [f32; 2] = [420.0, 900.0];

/// 40 folders of 3 painted layers (every other one masked) plus 40 fills: 200 layers at 2048².
fn big_document() -> OpenDocument {
    let res = 2048;
    let mut set = TextureSet::new("s", "Body", (res, res)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    let format = ChannelSpec::new(Channel::Roughness).layer_format();
    for g in 0..40 {
        let mut children = Vec::new();
        for k in 0..3 {
            let mut img = TiledImage::new(res, res, format).expect("image");
            for i in 0..64 {
                img.set((g * 40 + i) % res, (k * 300 + i) % res, [0.7, 0.7, 0.7, 1.0]);
            }
            let mut l =
                Layer::new(format!("p{g}_{k}"), format!("Paint {g}.{k}"), LayerKind::Paint(PaintContent { images: vec![(Channel::Roughness, img.clone())] }))
                    .with_blend(Channel::Roughness, BlendMode::Normal, 1.0);
            if k % 2 == 0 {
                l = l.with_mask(img);
            }
            children.push(l);
        }
        set.layers.push(Layer::new(format!("g{g}"), format!("Folder {g}"), LayerKind::Group(children)).with_blend(Channel::Roughness, BlendMode::Normal, 1.0));
        let fill = materialcraft::paint::Fill::uniform(vec![(Channel::Roughness, [0.3; 4])]);
        set.layers.push(Layer::new(format!("f{g}"), format!("Fill {g}"), LayerKind::Fill(fill)).with_blend(Channel::Roughness, BlendMode::Normal, 0.5));
    }
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    assert_eq!(doc.texture_sets[0].walk_layers().count(), 200);
    OpenDocument::new(doc, PathBuf::from("big.mcpaint"), None)
}

fn input(events: Vec<Event>) -> RawInput {
    RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(SCREEN[0], SCREEN[1]))), events, ..Default::default() }
}

/// One frame of the panel (layout, drawing, tessellation); returns its time and the events it sent.
fn frame(ctx: &egui::Context, panel: &mut LayerPanel, doc: &OpenDocument, events: Vec<Event>) -> (Duration, Vec<PanelEvent>) {
    let mut out = Vec::new();
    let t = Instant::now();
    let mut full = ctx.run_ui(input(events), |ui| panel.show(ui, doc, &mut out));
    // Tessellating is part of a frame's cost on the UI thread too.
    let _ = ctx.tessellate(std::mem::take(&mut full.shapes), full.pixels_per_point);
    full.textures_delta.clear();
    (t.elapsed(), out)
}

fn click(at: egui::Pos2) -> Vec<Vec<Event>> {
    vec![
        vec![Event::PointerMoved(at)],
        vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() }],
        vec![Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Default::default() }],
    ]
}

#[test]
fn a_200_layer_stack_stays_interactive() {
    let doc = big_document();
    let ctx = egui::Context::default();
    let mut panel = LayerPanel::default();
    let mut times = Vec::new();
    // Frames while thumbnails are computed in the background, then a scroll through the whole list.
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        times.push(frame(&ctx, &mut panel, &doc, Vec::new()).0);
        if panel.thumbs.cached() >= 10 && times.len() > 5 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(panel.thumbs.cached() >= 10, "thumbnails of the visible rows arrive ({} so far)", panel.thumbs.cached());
    for _ in 0..30 {
        let scroll = vec![
            Event::PointerMoved(pos2(200.0, 400.0)),
            Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -400.0), modifiers: Default::default(), phase: egui::TouchPhase::Move },
        ];
        times.push(frame(&ctx, &mut panel, &doc, scroll).0);
    }
    times.sort();
    let median = times[times.len() / 2];
    let worst = times[times.len() - 1];
    eprintln!("200 layers: {} frames, median {median:?}, worst {worst:?} (debug build: {})", times.len(), cfg!(debug_assertions));
    // Generous for unoptimised test builds; the release build is far below a 60 Hz frame.
    let budget = if cfg!(debug_assertions) { Duration::from_millis(150) } else { Duration::from_millis(16) };
    assert!(median < budget, "median frame {median:?}");
    assert!(worst < budget * 4, "worst frame {worst:?}");
}

#[test]
fn clicks_select_and_the_drop_target_api_finds_rows() {
    let doc = big_document();
    let ctx = egui::Context::default();
    let mut panel = LayerPanel::default();
    frame(&ctx, &mut panel, &doc, Vec::new());
    frame(&ctx, &mut panel, &doc, Vec::new());
    let blur = DropPayload::Effect(EffectAsset { name: "Blur".into(), kind: AssetEffect::Filter(Procedural::new("blur")) });
    // Scan down the panel for the first row: the top of the stack is the fill "f39".
    let onto = (0..SCREEN[1] as i32).map(|y| pos2(300.0, y as f32)).find(|p| matches!(panel.target_at(doc.doc(), *p, &blur), Some(DropTarget::OntoLayer(_))));
    let at = onto.expect("a row");
    assert_eq!(panel.target_at(doc.doc(), at, &blur), Some(DropTarget::OntoLayer(LayerRef::new("s", "f39"))));
    let edge = panel.target_at(doc.doc(), at - vec2(0.0, 5.0), &blur);
    assert_eq!(edge, Some(DropTarget::Between { set: "s".into(), at: materialcraft::paint::edit::LayerSlot::top(80) }), "above the top row");
    // Outside the list nothing is a target; hovering sets the marker state.
    assert_eq!(panel.target_at(doc.doc(), pos2(300.0, SCREEN[1] + 50.0), &blur), None);
    panel.hover(Some(blur));
    frame(&ctx, &mut panel, &doc, vec![Event::PointerMoved(at)]);
    panel.hover(None);
    // A click on the row selects the layer.
    for events in click(at + vec2(-100.0, 0.0)) {
        frame(&ctx, &mut panel, &doc, events);
    }
    assert_eq!(panel.selection.ids(), ["f39"]);
    assert!(!panel.paintable(doc.doc()), "a fill layer's content can't be painted");
    assert_eq!(panel.action(doc.doc(), LayerAction::Duplicate), Some(LayerEvent::Duplicate { set: "s".into(), ids: vec!["f39".into()] }));
}
