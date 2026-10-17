use std::time::{Duration, Instant};

use eframe::egui;
use materialcraft::diag::{Area, Collector, Entry, Filter, Level, LevelMask, RingBuffer, Sink, Tab, Target};

use super::index::LogIndex;
use super::rows::{Follow, OVERSCAN, is_at_bottom, row_window};
use super::selection::{Click, Selection};
use super::{LogPanel, text_of};

fn history(entries: &[(Level, Area, &str)]) -> RingBuffer {
    let mut h = RingBuffer::new(100);
    for (level, area, text) in entries {
        h.push(Entry::new(*level, *area, *text));
    }
    h
}

fn texts(index: &LogIndex, h: &RingBuffer) -> Vec<String> {
    index.shown().iter().filter_map(|id| h.get(*id)).map(|e| e.text.clone()).collect()
}

#[test]
fn tabs_split_the_areas_as_decided() {
    let h = history(&[
        (Level::Info, Area::Project, "opened"),
        (Level::Error, Area::Gpu, "shader"),
        (Level::Warning, Area::Import, "approximated"),
        (Level::Warning, Area::Checks, "layer left out"),
        (Level::Info, Area::Bake, "baked AO"),
    ]);
    let mut index = LogIndex::new(Filter::all().in_tab(Tab::General));
    index.sync(&h);
    assert_eq!(texts(&index, &h), ["opened", "shader"], "GPU messages go under General (VQ14)");
    index.set_filter(Filter::all().in_tab(Tab::Assets), &h);
    assert_eq!(texts(&index, &h), ["approximated", "layer left out"]);
    index.set_filter(Filter::all().in_tab(Tab::Bake), &h);
    assert_eq!(texts(&index, &h), ["baked AO"]);
    let (general, assets) = (index.counts(Tab::General), index.counts(Tab::Assets));
    assert_eq!((general.error, general.info, assets.warning, index.counts(Tab::Bake).info), (1, 1, 2, 1));
}

#[test]
fn level_filter_and_search_combine() {
    let h = history(&[
        (Level::Debug, Area::General, "noise"),
        (Level::Info, Area::General, "Saved project"),
        (Level::Warning, Area::General, "Export slow"),
        (Level::Error, Area::General, "export FAILED"),
    ]);
    let mut index = LogIndex::new(Filter::all().levels(LevelMask::at_least(Level::Info)).in_tab(Tab::General));
    index.sync(&h);
    assert_eq!(texts(&index, &h), ["Saved project", "Export slow", "export FAILED"], "debug is hidden by default");
    index.set_filter(index.filter().clone().containing("export"), &h);
    assert_eq!(texts(&index, &h), ["Export slow", "export FAILED"], "search ignores case");
    index.set_filter(index.filter().clone().levels(LevelMask::NONE.with(Level::Error, true)), &h);
    assert_eq!(texts(&index, &h), ["export FAILED"]);
    index.set_filter(index.filter().clone().containing("nothing like it"), &h);
    assert_eq!(index.len(), 0);
}

#[test]
fn the_index_follows_the_history_incrementally() {
    let mut h = RingBuffer::new(4);
    let mut index = LogIndex::new(Filter::all().in_tab(Tab::General));
    for i in 0..3 {
        h.push(Entry::new(Level::Info, Area::General, format!("m{i}")));
    }
    assert_eq!(index.sync(&h), 3);
    assert_eq!(index.sync(&h), 0, "nothing new, nothing added");
    for i in 3..7 {
        h.push(Entry::new(Level::Warning, Area::General, format!("m{i}")));
    }
    h.push(Entry::new(Level::Info, Area::Bake, "other tab"));
    assert_eq!(index.sync(&h), 3, "m4..m6 arrive; the bake message is in another tab");
    assert_eq!(texts(&index, &h), ["m4", "m5", "m6"], "entries the history dropped are forgotten");
    let general = index.counts(Tab::General);
    assert_eq!((general.info, general.warning, index.counts(Tab::Bake).info), (0, 3, 1), "counts follow the drops");
}

#[test]
fn clear_hides_what_was_there_and_keeps_later_messages() {
    let mut h = history(&[(Level::Info, Area::General, "old")]);
    let mut index = LogIndex::new(Filter::all().in_tab(Tab::General));
    index.sync(&h);
    index.clear();
    assert_eq!((index.len(), index.counts(Tab::General).total()), (0, 0));
    index.sync(&h);
    assert_eq!(index.len(), 0, "a sync does not bring cleared entries back");
    index.set_filter(Filter::all().in_tab(Tab::General).containing("o"), &h);
    assert_eq!(index.len(), 0, "nor does a filter change");
    index.set_filter(Filter::all().in_tab(Tab::General), &h);
    h.push(Entry::new(Level::Info, Area::General, "new"));
    index.sync(&h);
    assert_eq!(texts(&index, &h), ["new"]);
    assert_eq!(h.len(), 2, "the session history keeps everything");
}

#[test]
fn a_new_history_starts_the_index_over() {
    let h = history(&[(Level::Info, Area::General, "a"), (Level::Info, Area::General, "b")]);
    let mut index = LogIndex::new(Filter::all().in_tab(Tab::General));
    index.sync(&h);
    let fresh = history(&[(Level::Info, Area::General, "c")]);
    index.sync(&fresh);
    assert_eq!(texts(&index, &fresh), ["c"]);
}

#[test]
fn selection_by_click_toggle_and_range_and_copy_as_text() {
    let h =
        history(&[(Level::Info, Area::General, "a"), (Level::Info, Area::Bake, "x"), (Level::Info, Area::General, "b"), (Level::Error, Area::General, "c")]);
    let mut index = LogIndex::new(Filter::all().in_tab(Tab::General));
    index.sync(&h);
    let ids: Vec<_> = index.shown().iter().copied().collect();
    let mut sel = Selection::default();
    sel.click(ids[0], Click::Plain, &index);
    sel.click(ids[2], Click::Range, &index);
    assert_eq!(sel.ids().collect::<Vec<_>>(), ids, "a range takes the listed rows only");
    sel.click(ids[1], Click::Toggle, &index);
    assert_eq!(sel.len(), 2);
    let text = text_of(&h, sel.ids().collect());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].ends_with("[general] a") && lines[1].contains("error") && lines[1].ends_with("[general] c"), "{text}");
    sel.click(ids[1], Click::Plain, &index);
    assert_eq!(sel.ids().collect::<Vec<_>>(), [ids[1]]);
}

#[test]
fn the_row_window_covers_the_viewport_and_nothing_more() {
    assert_eq!(row_window(0, 18.0, 0.0, 400.0), 0..0);
    assert_eq!(row_window(100_000, 0.0, 0.0, 400.0), 0..0);
    assert_eq!(row_window(100_000, 18.0, 0.0, 400.0), 0..(23 + OVERSCAN));
    let mid = row_window(100_000, 18.0, 18.0 * 50_000.0 + 9.0, 400.0);
    assert_eq!(mid, (50_000 - OVERSCAN)..(50_000 + 23 + OVERSCAN));
    assert_eq!(row_window(10, 18.0, 1e9, 400.0), 10..10, "scrolled past the end: nothing");
    assert_eq!(row_window(10, 18.0, -50.0, 400.0), 0..10);
    assert_eq!(row_window(10, 18.0, f32::NAN, 400.0), 0..0);
}

#[test]
fn auto_scroll_pauses_when_scrolled_up_and_resumes_at_the_bottom() {
    let mut f = Follow::default();
    assert!(f.sticks());
    f.observe(true, false);
    assert!(f.sticks());
    f.observe(false, true);
    assert!(!f.sticks() && f.is_paused(), "scrolling up pauses");
    f.observe(false, false);
    assert!(f.is_paused(), "new messages don't resume it");
    f.observe(true, true);
    assert!(f.sticks(), "back at the bottom it follows again");
    f.observe(false, true);
    f.resume();
    assert!(f.sticks());
    f.enabled = false;
    assert!(!f.sticks() && !f.is_paused());
    assert!(is_at_bottom(10, 20.0, 0.0, 400.0), "a short list is always at the bottom");
    assert!(is_at_bottom(100, 20.0, 1600.0, 400.0) && !is_at_bottom(100, 20.0, 1500.0, 400.0));
}

/// Runs one frame of the panel in a headless egui context (800x600 points); returns the time it took
/// (the panel's layout and painting plus egui's tessellation) and the reveal events.
fn frame(ctx: &egui::Context, panel: &mut LogPanel, history: &RingBuffer) -> (Duration, Vec<super::PanelEvent>) {
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
    let mut events = Vec::new();
    let start = Instant::now();
    let mut out = ctx.run_ui(input, |ui| panel.show(ui, history, &mut events));
    let _ = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
    out.textures_delta.clear();
    (start.elapsed(), events)
}

fn burst_entry(i: usize) -> Entry {
    let (level, area) = match i % 7 {
        0 => (Level::Error, Area::Gpu),
        1 | 2 => (Level::Warning, Area::Import),
        3 => (Level::Info, Area::Bake),
        _ => (Level::Info, Area::General),
    };
    let e = Entry::new(level, area, format!("burst message {i}: texture set {} layer {}", i % 13, i % 101));
    if i.is_multiple_of(5) { e.with_target(Target::Layer { texture_set: format!("ts{}", i % 13), layer: format!("l{}", i % 101) }) } else { e }
}

fn report(name: &str, times: &mut [Duration]) -> Duration {
    times.sort_unstable();
    let (median, worst) = (times[times.len() / 2], times[times.len() - 1]);
    eprintln!("log panel, {name}: median {:.2} ms, worst {:.2} ms over {} frames", median.as_secs_f64() * 1e3, worst.as_secs_f64() * 1e3, times.len());
    worst
}

/// The gate (LG-08): a burst of 100 000 entries keeps the panel within a 60 fps frame. The history
/// here holds all 100 000 (ten times the editor's default), the panel shows the General tab with
/// auto-scroll on. Measured: the first frame (the whole burst indexed at once), frames that each
/// add a full collector inbox (4096 entries, the most one drain hands over) while the oldest drop,
/// and frames that change the search (a rescan of all 100 000). Times are asserted in release
/// builds only; debug builds check the row count.
#[test]
fn a_100k_burst_keeps_the_panel_within_a_frame() {
    let ctx = egui::Context::default();
    let mut panel = LogPanel::default();
    let mut h = RingBuffer::new(100_000);
    for i in 0..100_000 {
        h.push(burst_entry(i));
    }
    let _ = frame(&ctx, &mut panel, &h); // fonts are prepared on the first pass
    let mut panel = LogPanel::default();
    let (first, _) = frame(&ctx, &mut panel, &h);
    eprintln!("log panel, first frame with 100000 entries: {:.2} ms", first.as_secs_f64() * 1e3);
    assert!(panel.index.len() > 50_000);
    let visible_rows = (600.0 / 14.0) as usize + 2 * OVERSCAN + 2;
    assert!(panel.laid_out > 0 && panel.laid_out <= visible_rows, "{} rows laid out", panel.laid_out);

    let collector = Collector::default();
    let mut steady = Vec::new();
    for round in 0..40 {
        for i in 0..4096 {
            collector.emit(burst_entry(100_000 + round * 4096 + i));
        }
        collector.drain_into(&mut h);
        let (t, _) = frame(&ctx, &mut panel, &h);
        steady.push(t);
        assert!(panel.laid_out <= visible_rows);
    }
    let newest = h.last().map(|(id, _)| id);
    assert!(panel.index.shown().back().copied() <= newest && panel.follow.sticks(), "auto-scroll stays on through the burst");

    let mut search = Vec::new();
    for q in ["layer 7", "texture set 3", "burst", "nothing matches this", ""] {
        panel.search = q.to_owned();
        search.push(frame(&ctx, &mut panel, &h).0);
    }
    let worst = report("4096 new entries per frame", &mut steady).max(report("search over 100000", &mut search)).max(first);
    if !cfg!(debug_assertions) {
        assert!(worst < Duration::from_micros(16_667), "a frame took {worst:?}");
    }
}
