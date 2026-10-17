//! `--stroke-test`: once the document has settled, paints one red stroke across the middle of the view through
//! the same path a pointer stroke takes (a new paint layer on the first texture set, the paint tool, the stroke
//! gesture's input), so `--shot` can show the brush pipeline at work without a person at the mouse. With
//! `--stroke-helpers` the stroke is mirrored across X with the lazy mouse on and followed by a straight line.

use super::PaintMode;
use super::input::{PointerSample, StrokeInput};
use super::panels::PanelEvent;
use super::panels::layers::LayerAction;
use super::shell::tools::Tool;
use super::tools::PaintFrame;
use super::tools::helpers::{LazySettings, StrokeHelpers, SymmetrySettings};
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::Channel;
use materialcraft::paint::edit::ops::NewLayer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeTest {
    /// Waiting for the document, its maps and the picking scene.
    Waiting,
    /// A paint layer was asked for; waiting for the brush surface.
    Layer,
    Done,
}

/// The helpers `--stroke-helpers` paints with: mirror across object X at 0, lazy mouse of 12 points.
pub fn helpers() -> StrokeHelpers {
    StrokeHelpers { lazy: LazySettings { on: true, radius: 12.0 }, symmetry: SymmetrySettings { on: true, ..SymmetrySettings::default() } }
}

impl PaintMode {
    /// Advances the stroke test; true once it has painted (or given up).
    pub(super) fn run_stroke_test(&mut self, settled: bool) -> bool {
        let Some(stage) = self.stroke_test else { return true };
        match stage {
            StrokeTest::Done => return true,
            StrokeTest::Waiting if settled => {
                self.shell.tools.select(Tool::Paint);
                self.painting.brush.value_mut(Channel::BaseColor).value = [0.85, 0.12, 0.08, 1.0];
                self.painting.brush.value_mut(Channel::Roughness).value = [0.25; 4];
                self.apply(PanelEvent::LayerAction(LayerAction::New(NewLayer::Paint)));
                self.stroke_test = Some(StrokeTest::Layer);
            }
            StrokeTest::Layer => self.paint_test_stroke(),
            StrokeTest::Waiting => {}
        }
        false
    }

    fn paint_test_stroke(&mut self) {
        let (Some(doc), Some(camera)) = (self.document.as_mut(), self.viewport.camera()) else { return };
        let Some((target, set)) = self.layers.paint_target(doc.doc()) else {
            self.diag.warn(Area::General, "Stroke test: the new layer can't be painted.");
            self.stroke_test = Some(StrokeTest::Done);
            return;
        };
        if !self.painting.surface_ready(doc, set) {
            return;
        }
        let [w, h] = camera.viewport;
        let sample = |i: usize| {
            let t = i as f32 / 40.0;
            PointerSample { pos: [w * (0.3 + 0.4 * t), h * (0.5 + 0.08 * (t * 9.0).sin())], pressure: 1.0, time: f64::from(t) * 0.7 }
        };
        let mut input = vec![StrokeInput::Down(sample(0))];
        input.extend((1..=40).map(|i| StrokeInput::Move(sample(i))));
        input.push(StrokeInput::Up);
        if self.painting.brush.helpers.symmetry.on {
            let to = PointerSample { pos: [w * 0.42, h * 0.78], pressure: 1.0, time: 1.0 };
            input.extend([StrokeInput::Line { to, snap: true }, StrokeInput::Up]);
        }
        let frame = PaintFrame { doc, picking: &self.picking, camera: Some(camera), tool: Tool::Paint, target: Some((target, set)) };
        let report = self.painting.frame(input, frame);
        self.after_painting(report);
        let steps = self.document.as_ref().map_or(0, |d| d.history().len());
        self.diag.info(Area::General, format!("Stroke test: painted one stroke; the history has {steps} steps."));
        self.stroke_test = Some(StrokeTest::Done);
    }
}
