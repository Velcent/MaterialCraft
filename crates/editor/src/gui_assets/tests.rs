//! The tray headless: the gate's timings (a 10 000-asset library scrolls within a 60 fps frame, a new search
//! shows in the same frame), drags ending in a drop event, and the library service picking up new files.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use materialcraft::assets::{Asset, Format, Library, LibraryRoot, RootKind, Usage};
use materialcraft::diag::NullSink;

use super::*;
use crate::gui_settings::ThumbSize;

/// A library with one in-memory root of `n` assets spread over folders and usages.
fn library(n: usize) -> Library {
    let sink = NullSink;
    let mut lib = Library::new();
    lib.add_root(LibraryRoot::in_memory("mem", "Test library", RootKind::Session), &sink);
    let words = ["steel", "rust", "wood", "concrete", "leather", "paint", "dirt", "plastic"];
    let assets = (0..n).map(|i| {
        let usage = Usage::ALL[i % Usage::ALL.len()];
        let word = words[i % words.len()];
        let mut a = Asset::new(format!("{}/{}/{word}_{i:05}.mca", usage.folder(), word), Format::Graph, usage);
        a.tags = vec![word.to_owned(), format!("set{}", i % 13)];
        a
    });
    lib.set_assets(&materialcraft::assets::RootId::new("mem"), assets);
    lib
}

fn tray(n: usize, size: ThumbSize) -> AssetTray {
    let service = LibraryService::from_library(library(n), Arc::new(NullSink));
    let mut settings = LibrarySettings::default();
    settings.tray.thumbnails = size;
    AssetTray::with_service(service, ThumbCache::new(None), None, &settings)
}

/// One frame of the tray in a 400 x 800 panel; returns the time it took (layout, painting and tessellation).
fn frame(ctx: &egui::Context, tray: &mut AssetTray, input: egui::RawInput) -> (Duration, Vec<TrayEvent>) {
    let mut events = Vec::new();
    let start = Instant::now();
    let mut out = ctx.run_ui(input, |ui| {
        tray.show(ui, &mut events);
    });
    events.extend(tray.end_frame(ctx));
    let _ = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
    out.textures_delta.clear();
    (start.elapsed(), events)
}

fn screen() -> egui::RawInput {
    egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 800.0))), ..Default::default() }
}

fn wheel(dy: f32) -> egui::RawInput {
    let mut i = screen();
    i.events.push(egui::Event::PointerMoved(egui::pos2(300.0, 500.0)));
    i.events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, dy),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    i
}

fn worst(name: &str, times: &mut [Duration]) -> Duration {
    times.sort_unstable();
    let (median, worst) = (times[times.len() / 2], times[times.len() - 1]);
    eprintln!("asset tray, {name}: median {:.2} ms, worst {:.2} ms over {} frames", median.as_secs_f64() * 1e3, worst.as_secs_f64() * 1e3, times.len());
    worst
}

/// The gate: 10 000 assets scroll at 60 fps in every thumbnail size, and a changed search is shown in the frame
/// it was typed in, also within a 60 fps frame. Times are asserted in release builds only.
#[test]
fn ten_thousand_assets_scroll_and_search_within_a_frame() {
    let ctx = egui::Context::default();
    let mut slowest = Duration::ZERO;
    for size in [ThumbSize::Small, ThumbSize::Medium, ThumbSize::Large, ThumbSize::List] {
        let mut t = tray(10_000, size);
        let _ = frame(&ctx, &mut t, screen()); // fonts, first results
        assert_eq!(t.results.items.len(), 10_000);
        let mut times = Vec::new();
        let mut deepest = 0;
        for k in 0..60 {
            let dy = if k < 40 { -900.0 } else { 2500.0 };
            times.push(frame(&ctx, &mut t, wheel(dy)).0);
            deepest = deepest.max(t.first_cell.map_or(0, |(i, _)| i));
        }
        assert!(deepest > 100, "{size:?}: the wheel scrolled the grid (first asset in view reached {deepest})");
        slowest = slowest.max(worst(&format!("scrolling {size:?}"), &mut times));
    }

    let mut t = tray(10_000, ThumbSize::Medium);
    let _ = frame(&ctx, &mut t, screen());
    let mut times = Vec::new();
    for (q, expect) in [("rust", Some(1250)), ("!rust_00001", Some(1)), ("tag:set3 -wood", None), ("usage:filter", None), ("", Some(10_000))] {
        t.filter.query = q.to_owned();
        let (time, _) = frame(&ctx, &mut t, screen());
        times.push(time);
        assert!(t.results.is_current(t.service.generation(), &t.filter.search()), "{q}: the results are this frame's");
        if let Some(n) = expect {
            assert_eq!(t.results.items.len(), n, "{q}");
        }
    }
    t.filter.toggle(Usage::Material);
    times.push(frame(&ctx, &mut t, screen()).0);
    assert_eq!(t.results.items.len(), 10_000 / 9 + 1);
    slowest = slowest.max(worst("a new search", &mut times));
    if !cfg!(debug_assertions) {
        assert!(slowest < Duration::from_micros(16_667), "a frame took {slowest:?}");
    }
}

#[test]
fn dragging_a_material_out_ends_in_one_drop_event_with_ctrl_as_replace() {
    let ctx = egui::Context::default();
    let mut t = tray(20, ThumbSize::Large);
    t.filter.toggle(Usage::Material);
    let _ = frame(&ctx, &mut t, screen());
    let press = |pos: egui::Pos2, pressed: bool, mods: egui::Modifiers| {
        let mut i = screen();
        i.events.push(egui::Event::ModifiersChanged(mods));
        i.events.push(egui::Event::PointerMoved(pos));
        i.events.push(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: mods });
        i
    };
    let moved = |pos: egui::Pos2| {
        let mut i = screen();
        i.events.push(egui::Event::PointerMoved(pos));
        i
    };
    let start = t.first_cell.expect("a cell was drawn").1.center();
    let _ = frame(&ctx, &mut t, press(start, true, egui::Modifiers::NONE));
    for k in 1..6 {
        let _ = frame(&ctx, &mut t, moved(start + egui::vec2(0.0, 12.0 * k as f32)));
    }
    eprintln!("start {start:?} dragged {:?} first {:?}", ctx.dragged_id(), t.first_cell);
    assert!(t.dragged_payload().is_some(), "the drag started");
    let end = start + egui::vec2(-100.0, 80.0);
    let (_, events) = frame(&ctx, &mut t, press(end, false, egui::Modifiers::COMMAND));
    let dropped: Vec<_> = events.iter().filter(|e| matches!(e, TrayEvent::Dropped { .. })).collect();
    assert_eq!(dropped.len(), 1, "{events:?}");
    if let Some(TrayEvent::Dropped { item, payload, pos, replace }) = dropped.first() {
        assert_eq!(item.usage, Usage::Material);
        assert!(matches!(payload, DropPayload::Material(_)));
        assert_eq!(*pos, end);
        assert!(*replace);
    }
    assert!(t.dragged_payload().is_none());
}

#[test]
fn the_service_sees_files_added_to_a_library_folder() {
    let dir = std::env::temp_dir().join(format!("mc-tray-service-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let user = dir.join(roots::USER_FOLDER);
    assert!(roots::first_run(&user).is_some());
    std::fs::write(user.join("materials").join("brick.mca"), materialcraft::graph::EXAMPLE).expect("write");
    let service = LibraryService::spawn(roots::roots(Some(&user), &[], None), Arc::new(NullSink));
    let mut seen = 0;
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(30));
        seen = service.read().map_or(0, |l| l.catalogue().len());
        if seen == 1 {
            break;
        }
    }
    assert_eq!(seen, 1, "the first scan found the material");
    std::fs::write(user.join("textures").join("dots.png"), materialcraft::png::encode(4, 4, &[[0.5; 4]; 16], 8)).expect("write");
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(50));
        seen = service.read().map_or(0, |l| l.catalogue().len());
        if seen == 2 {
            break;
        }
    }
    assert_eq!(seen, 2, "the poll found the new texture (AS-19)");
    assert!(service.generation() >= 2);
    drop(service);
    let _ = std::fs::remove_dir_all(&dir);
}
