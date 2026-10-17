//! Headless gate for the diagnostics model (roadmap step 4): bounded buffers, bursts from several
//! threads, filters and queries.

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use materialcraft::diag::{Area, Collector, Entry, EntryId, Filter, Level, LevelMask, NullSink, RingBuffer, Sink, SinkExt, Tab, Target};

fn entry(level: Level, area: Area, text: &str) -> Entry {
    Entry::new(level, area, text)
}

#[test]
fn the_buffer_stays_bounded_and_keeps_the_newest_entries() {
    let mut buf = RingBuffer::new(100);
    for i in 0..1000 {
        buf.push(entry(Level::Info, Area::General, &format!("m{i}")));
        assert!(buf.len() <= 100);
    }
    assert_eq!(buf.len(), 100);
    assert_eq!(buf.dropped(), 900);
    let texts: Vec<_> = buf.iter().map(|(_, e)| e.text.clone()).collect();
    assert_eq!(texts.first().map(String::as_str), Some("m900"));
    assert_eq!(texts.last().map(String::as_str), Some("m999"));
    // Ids are stable: dropped ids are gone, surviving ids still resolve.
    assert!(buf.get(EntryId(899)).is_none());
    assert_eq!(buf.get(EntryId(950)).map(|e| e.text.as_str()), Some("m950"));
    assert!(buf.get(EntryId(1000)).is_none());
    assert_eq!(buf.last().map(|(id, _)| id), Some(EntryId(999)));
}

#[test]
fn a_zero_capacity_is_raised_to_one() {
    let mut buf = RingBuffer::new(0);
    buf.push(entry(Level::Info, Area::General, "a"));
    buf.push(entry(Level::Info, Area::General, "b"));
    assert_eq!(buf.len(), 1);
    assert_eq!(buf.iter().next().map(|(_, e)| e.text.as_str()), Some("b"));
}

#[test]
fn clearing_keeps_ids_counting_and_since_returns_new_arrivals() {
    let mut buf = RingBuffer::new(10);
    for i in 0..5 {
        buf.push(entry(Level::Info, Area::General, &format!("m{i}")));
    }
    let mark = buf.next_id();
    buf.clear();
    assert!(buf.is_empty() && buf.get(EntryId(0)).is_none());
    let id = buf.push(entry(Level::Info, Area::General, "after"));
    assert_eq!(id, EntryId(5));
    assert_eq!(buf.since(mark).count(), 1);
    assert_eq!(buf.since(EntryId(u64::MAX)).count(), 0);
    assert_eq!(buf.since(EntryId(0)).count(), 1);
}

fn burst(collector: &Arc<Collector>, threads: usize, per_thread: usize) {
    let workers: Vec<_> = (0..threads)
        .map(|t| {
            let sink = Arc::clone(collector);
            thread::spawn(move || {
                for i in 0..per_thread {
                    sink.info(Area::Bake, format!("t{t} #{i}"));
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
}

/// (thread, index) from a burst message `t<thread> #<index>`.
fn parse(text: &str) -> (usize, usize) {
    let (t, i) = text.trim_start_matches('t').split_once(" #").unwrap();
    (t.parse().unwrap(), i.parse().unwrap())
}

#[test]
fn a_burst_of_100k_from_several_threads_does_not_block_or_lose_the_newest() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 12_500;
    let collector = Arc::new(Collector::new(1000));
    let start = Instant::now();
    // Nobody drains while the burst runs: producers must still finish promptly and memory stays bounded.
    burst(&collector, THREADS, PER_THREAD);
    assert!(start.elapsed() < Duration::from_secs(20), "burst took {:?}", start.elapsed());
    assert_eq!(collector.pending(), 1000);
    collector.info(Area::General, "after the burst");

    let mut history = RingBuffer::new(500);
    let added = collector.drain_into(&mut history);
    assert_eq!(added, 1001, "the 1000 kept entries plus one drop note");
    assert_eq!(history.len(), 500);
    assert_eq!(collector.pending(), 0);
    assert_eq!(history.last().map(|(_, e)| e.text.as_str()), Some("after the burst"));

    // Only the oldest entries are dropped, so what survives of each thread is a gap-free run ending in
    // that thread's final message.
    let collector = Arc::new(Collector::new(1000));
    burst(&collector, THREADS, PER_THREAD);
    let mut history = RingBuffer::new(2000);
    collector.drain_into(&mut history);
    let (_, note) = history.iter().next().unwrap();
    assert_eq!(note.level, Level::Warning);
    assert!(note.text.starts_with(&format!("{} messages were dropped", THREADS * PER_THREAD - 1000)), "{}", note.text);
    let mut kept: Vec<Vec<usize>> = vec![Vec::new(); THREADS];
    for (_, e) in history.iter().skip(1) {
        let (t, i) = parse(&e.text);
        kept[t].push(i);
    }
    assert_eq!(kept.iter().map(Vec::len).sum::<usize>(), 1000);
    for run in kept.iter().filter(|r| !r.is_empty()) {
        assert_eq!(run.last(), Some(&(PER_THREAD - 1)));
        assert!(run.windows(2).all(|w| w[1] == w[0] + 1));
    }
}

#[test]
fn draining_while_producers_run_keeps_every_message_when_the_inbox_is_large_enough() {
    let collector = Arc::new(Collector::new(200_000));
    let mut history = RingBuffer::new(200_000);
    let workers: Vec<_> = (0..4)
        .map(|t| {
            let sink = Arc::clone(&collector);
            thread::spawn(move || {
                for i in 0..25_000 {
                    sink.warn(Area::Gpu, format!("t{t} #{i}"));
                }
            })
        })
        .collect();
    while !workers.iter().all(|w| w.is_finished()) {
        collector.drain_into(&mut history);
    }
    for w in workers {
        w.join().unwrap();
    }
    collector.drain_into(&mut history);
    assert_eq!(history.len(), 100_000);
    assert_eq!(history.dropped(), 0);
    let mut next = [0usize; 4];
    for (_, e) in history.iter() {
        let (t, i) = parse(&e.text);
        assert_eq!(i, next[t], "per producer, order is preserved");
        next[t] += 1;
    }
    assert_eq!(next, [25_000; 4]);
}

fn sample() -> RingBuffer {
    let mut buf = RingBuffer::default();
    buf.push(entry(Level::Info, Area::Project, "Opened project"));
    buf.push(entry(Level::Warning, Area::Import, "Unknown blend mode, using Normal"));
    buf.push(entry(Level::Error, Area::Bake, "AO bake failed: no mesh").with_target(Target::BakeSetting("ao.rays".into())));
    buf.push(entry(Level::Error, Area::Gpu, "Shader compile failed"));
    buf.push(entry(Level::Debug, Area::Gpu, "glow: context created"));
    buf.push(entry(Level::Warning, Area::Checks, "Layer has no channels").with_target(Target::Layer { texture_set: "body".into(), layer: "rust".into() }));
    buf
}

fn texts<'a>(buf: &'a RingBuffer, f: &'a Filter) -> Vec<&'a str> {
    buf.query(f).map(|(_, e)| e.text.as_str()).collect()
}

#[test]
fn filters_by_level_tab_area_target_and_text() {
    let buf = sample();
    assert_eq!(buf.query(&Filter::all()).count(), 6);
    assert_eq!(texts(&buf, &Filter::all().levels(LevelMask::at_least(Level::Error))), ["AO bake failed: no mesh", "Shader compile failed"]);
    let warnings_and_up = LevelMask::ALL.with(Level::Debug, false).with(Level::Info, false);
    assert_eq!(texts(&buf, &Filter::all().levels(warnings_and_up).in_tab(Tab::Assets)), ["Unknown blend mode, using Normal", "Layer has no channels"]);
    // GPU messages live under General (VQ14), bake under its own tab.
    assert_eq!(texts(&buf, &Filter::all().in_tab(Tab::General)), ["Opened project", "Shader compile failed", "glow: context created"]);
    assert_eq!(texts(&buf, &Filter::all().in_tab(Tab::Bake)), ["AO bake failed: no mesh"]);
    assert_eq!(texts(&buf, &Filter::all().in_area(Area::Gpu).levels(LevelMask::at_least(Level::Info))), ["Shader compile failed"]);
    assert_eq!(texts(&buf, &Filter::all().about(Target::BakeSetting("ao.rays".into()))), ["AO bake failed: no mesh"]);
    assert_eq!(texts(&buf, &Filter::all().containing("BLEND")), ["Unknown blend mode, using Normal"]);
    assert!(texts(&buf, &Filter::all().levels(LevelMask::NONE)).is_empty());
    // Newest first, for a panel that lists in reverse.
    assert_eq!(buf.query(&Filter::all()).next_back().map(|(_, e)| e.area), Some(Area::Checks));
}

#[test]
fn counts_and_text_export() {
    let buf = sample();
    let c = buf.counts(&Filter::all());
    assert_eq!((c.debug, c.info, c.warning, c.error, c.total()), (1, 1, 2, 2, 6));
    assert_eq!(buf.counts(&Filter::all().in_tab(Tab::Assets)).get(Level::Warning), 2);

    let mut one = RingBuffer::new(4);
    one.push(Entry::new(Level::Warning, Area::Checks, "Layer has no channels\n").at(UNIX_EPOCH).with_target(Target::TextureSet("body".into())));
    assert_eq!(one.to_text(&Filter::all()), "1970-01-01T00:00:00.000Z warning [checks] Layer has no channels (texture set body)\n");
}

#[test]
fn every_area_has_a_tab_and_names_round_trip() {
    for area in Area::ALL {
        assert!(Tab::ALL.contains(&area.tab()));
    }
    for level in Level::ALL {
        assert_eq!(Level::parse(level.name()), Some(level));
    }
    assert_eq!(Level::parse(" WARN "), Some(Level::Warning));
    assert_eq!(Level::parse("loud"), None);
}

#[test]
fn producers_take_any_sink_including_dyn_and_null() {
    fn produce(sink: &dyn Sink) {
        sink.about(Level::Error, Area::Assets, Target::Asset("brushes/chalk".into()), "missing file");
        sink.debug(Area::Assets, "scan done");
    }
    produce(&NullSink);
    let collector = Collector::default();
    produce(&collector);
    let mut history = RingBuffer::default();
    collector.drain_into(&mut history);
    assert_eq!(history.len(), 2);
    assert_eq!(history.iter().next().and_then(|(_, e)| e.target.clone()), Some(Target::Asset("brushes/chalk".into())));
}
