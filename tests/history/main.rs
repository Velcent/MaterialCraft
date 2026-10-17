//! The history gate: 1,000 random structural and pixel edits undo to a byte-identical document and redo back,
//! memory stays under the budget, and coalescing makes one stroke or one drag one step.

mod random_edits;

use materialcraft::history::{CoalesceKey, PaintHistory};
use materialcraft::paint::edit::{Edit, ImageRef, LayerProperty, LayerRef, TileCapture};
use materialcraft::paint::{Channel, Document, LayerImage, TILE_SIZE, format};
use random_edits::{Generator, Op, document, run};
use std::collections::HashMap;

fn saved(doc: &Document) -> Vec<u8> {
    let mut bytes = Vec::new();
    format::save(doc, &mut bytes).unwrap();
    bytes
}

/// The document state before and after every step the history holds, by step serial.
#[derive(Default)]
struct States {
    before: HashMap<u64, Document>,
    after: HashMap<u64, Document>,
}

impl States {
    /// Notes the state after an operation that may have added or extended the newest step.
    fn recorded(&mut self, history: &PaintHistory, before: &Document, now: &Document) {
        if let Some(top) = history.steps().last().filter(|s| s.done && history.done() == history.len()) {
            self.before.entry(top.serial).or_insert_with(|| before.clone());
            self.after.insert(top.serial, now.clone());
        }
    }

    /// The state the history's cursor stands at.
    fn expected(&self, history: &PaintHistory) -> Option<&Document> {
        let steps: Vec<u64> = history.steps().map(|s| s.serial).collect();
        match history.done() {
            0 => self.before.get(steps.first()?),
            n => self.after.get(steps.get(n - 1)?),
        }
    }
}

/// Runs `count` random edits (plus undo, redo and jumps in `history_percent` of the operations) and checks every state the history
/// returns to. Returns the document, history and states.
fn random_session(seed: u64, count: usize, budget: usize, history_percent: u64) -> (Document, PaintHistory, States, Document) {
    let initial = document((260, 200));
    let mut doc = initial.clone();
    let mut history = PaintHistory::new(budget);
    let mut states = States::default();
    let mut generator = Generator::new(seed);
    let (mut edits, mut rejected) = (0, 0);
    while edits < count {
        let op = generator.op(&doc, history.len(), history_percent);
        let is_edit = matches!(op, Op::Edit { .. } | Op::Stroke { .. });
        let before = doc.clone();
        let (len, top) = (history.len(), history.steps().last().map(|s| s.serial));
        if is_edit {
            edits += 1;
            if !run(op, &mut doc, &mut history) {
                rejected += 1;
                if history.len() == len && history.steps().last().map(|s| s.serial) == top {
                    assert_eq!(doc, before, "a rejected edit changed the document");
                }
            }
            states.recorded(&history, &before, &doc);
        } else {
            run(op, &mut doc, &mut history);
            if let Some(expected) = states.expected(&history) {
                assert!(doc == *expected, "seed {seed}, edit {edits}: undo/redo did not return to the recorded state");
            }
        }
        doc.validate().unwrap_or_else(|e| panic!("seed {seed}, edit {edits}: {e}"));
        let retained = history.retained_bytes(&doc);
        assert!(retained <= budget || history.len() == 1, "seed {seed}, edit {edits}: {retained} bytes kept, budget {budget}");
    }
    eprintln!(
        "seed {seed}: {count} edits, {rejected} rejected, {} steps kept, {} dropped, {} layers, retained {} bytes",
        history.len(),
        history.dropped(),
        doc.texture_sets.iter().map(|s| s.walk_layers().count()).sum::<usize>(),
        history.retained_bytes(&doc)
    );
    assert!(rejected < count / 4, "{rejected} of {count} edits rejected: the generator is too weak");
    (doc, history, states, initial)
}

#[test]
fn thousand_random_edits_undo_to_identical_bytes_and_redo_back() {
    let (mut doc, mut history, states, initial) = random_session(0x5eed_1234, 1000, usize::MAX, 0);
    assert!(history.len() > 900, "{} steps: every edit that was not rejected is a step", history.len());
    let end = saved(&doc);
    let end_doc = doc.clone();
    // Step back one at a time to the start, checking every state, then forward again.
    while history.can_undo() {
        history.undo(&mut doc).unwrap();
        assert!(doc == *states.expected(&history).unwrap(), "undo to step {}", history.done());
    }
    assert!(doc == initial);
    assert_eq!(saved(&doc), saved(&initial), "undo all: .mcpaint bytes differ from the start");
    while history.can_redo() {
        history.redo(&mut doc).unwrap();
        assert!(doc == *states.expected(&history).unwrap(), "redo to step {}", history.done());
    }
    assert!(doc == end_doc);
    assert_eq!(saved(&doc), end, "redo all: .mcpaint bytes differ from the end");
}

#[test]
fn undo_redo_and_jumps_between_random_edits_return_to_recorded_states() {
    for seed in [7, 8] {
        let (mut doc, mut history, states, _) = random_session(seed, 500, usize::MAX, 15);
        assert!(history.dropped() > 0, "seed {seed}: new edits after undo drop the undone steps");
        history.jump_to(&mut doc, 0).unwrap();
        assert!(doc == *states.expected(&history).unwrap(), "seed {seed}: oldest state");
        history.jump_to(&mut doc, history.len()).unwrap();
        assert!(doc == *states.expected(&history).unwrap(), "seed {seed}: newest state");
    }
}

#[test]
fn several_seeds_with_a_small_budget_stay_under_it() {
    let budget = 3 << 20;
    for seed in [1, 2, 3] {
        let (mut doc, mut history, states, _) = random_session(seed, 400, budget, 10);
        assert!(history.dropped() > 0);
        let expected = |h: &PaintHistory| states.expected(h).cloned();
        history.jump_to(&mut doc, 0).unwrap();
        assert!(doc == expected(&history).unwrap(), "seed {seed}: oldest kept state");
        history.jump_to(&mut doc, history.len()).unwrap();
        assert!(doc == expected(&history).unwrap(), "seed {seed}: newest state");
    }
}

#[test]
fn one_stroke_is_one_step_holding_each_old_tile_once() {
    let mut doc = document((512, 512));
    let image = ImageRef::new(LayerRef::new("set0", "l0"), LayerImage::Paint(Channel::Roughness));
    Edit::WritePixels { image: image.clone(), rect: materialcraft::paint::Rect { x: 0, y: 0, w: 512, h: 512 }, pixels: vec![[0.5; 4]; 512 * 512] }
        .apply(&mut doc)
        .unwrap();
    let start = doc.clone();
    let mut history = PaintHistory::default();
    let stroke = Some(CoalesceKey(1));
    // 100 dabs along a diagonal: tiles (0,0) … (3,3), each touched by many dabs.
    for i in 0..100u32 {
        let capture = TileCapture::begin(&doc, image.clone()).unwrap();
        let target = doc.texture_set_mut("set0").unwrap().layer_mut("l0").unwrap().image_mut(image.image).unwrap();
        target.set(i * 5, i * 5, [1.0; 4]);
        history.record(&doc, "Brush stroke", stroke, capture.finish(&doc).unwrap());
    }
    history.close();
    assert_eq!(history.len(), 1);
    let tile_bytes = (TILE_SIZE * TILE_SIZE) as usize * Channel::Roughness.default_format().bytes_per_pixel();
    assert_eq!(history.retained_bytes(&doc), 4 * tile_bytes, "the four old tiles, once each");
    let end = doc.clone();
    history.undo(&mut doc).unwrap();
    assert!(doc == start);
    assert_eq!(saved(&doc), saved(&start));
    history.redo(&mut doc).unwrap();
    assert!(doc == end);

    // A slider drag: one step, and undo returns to the value before the drag.
    let layer = LayerRef::new("set0", "l1");
    for i in 0..50 {
        let edit = Edit::SetLayer { layer: layer.clone(), property: LayerProperty::Opacity(i as f32 / 50.0) };
        history.apply(&mut doc, "Opacity", Some(CoalesceKey(2)), edit).unwrap();
    }
    history.close();
    assert_eq!(history.len(), 2);
    history.undo(&mut doc).unwrap();
    assert_eq!(doc.texture_set("set0").unwrap().layer("l1").unwrap().opacity, 1.0);
    assert!(doc == end);
}
