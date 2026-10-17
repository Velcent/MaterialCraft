//! Pointer input for the brush tools (PT-56): the viewport records each frame's stroke gesture as
//! [`StrokeInput`]s (every pointer move egui saw, not only the frame's last position) with pen pressure where the
//! platform reports it (touch force; a mouse is full pressure). Tablet tilt and rotation come with roadmap step 35.
//! A straight-line gesture (PT-31) gives one [`StrokeInput::Line`] at the click and ignores the drag after it.

use eframe::egui::{self, Event, Pos2, Rect, TouchPhase};

/// One pointer sample in view coordinates (UI points from the view's top-left corner).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSample {
    pub pos: [f32; 2],
    /// 0 … 1; 1 for a mouse.
    pub pressure: f32,
    /// Seconds (egui's input clock).
    pub time: f64,
}

/// What the stroke gesture did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrokeInput {
    Down(PointerSample),
    Move(PointerSample),
    Up,
    /// A straight line from the previous stroke's end to `to`, snapped to angle steps if `snap`.
    Line {
        to: PointerSample,
        snap: bool,
    },
}

/// The stroke gesture's input since the tools last took it.
#[derive(Default)]
pub struct PointerLog {
    events: Vec<StrokeInput>,
    down: bool,
    /// The gesture held is a straight line: its moves are not strokes.
    line: bool,
    pressure: f32,
}

impl PointerLog {
    /// Records this frame's input of the view at `rect`; `stroking` says whether the stroke gesture is held
    /// (pressed in the view with the stroke binding), `started` / `stopped` whether it began or ended this frame,
    /// `line` whether it is a straight-line gesture (and whether that snaps).
    pub fn record(&mut self, input: &egui::InputState, rect: Rect, stroking: bool, started: bool, stopped: bool, line: Option<bool>) {
        let time = input.time;
        for e in &input.events {
            if let Event::Touch { force: Some(f), phase, .. } = e
                && matches!(phase, TouchPhase::Start | TouchPhase::Move)
                && f.is_finite()
            {
                self.pressure = f.clamp(0.0, 1.0);
            }
        }
        let pressure = if self.pressure > 0.0 { self.pressure } else { 1.0 };
        let local = |p: Pos2| PointerSample { pos: [p.x - rect.min.x, p.y - rect.min.y], pressure, time };
        if started && !self.down {
            let at = input.pointer.press_origin().or(input.pointer.interact_pos());
            if let Some(p) = at {
                self.events.push(match line {
                    Some(snap) => StrokeInput::Line { to: local(p), snap },
                    None => StrokeInput::Down(local(p)),
                });
                self.down = true;
                self.line = line.is_some();
            }
        }
        if self.down && !self.line && (stroking || stopped) {
            for e in &input.events {
                if let Event::PointerMoved(p) = e {
                    self.events.push(StrokeInput::Move(local(*p)));
                }
            }
        }
        if self.down && (stopped || !stroking) {
            self.events.push(StrokeInput::Up);
            self.down = false;
            self.line = false;
            self.pressure = 0.0;
        }
    }

    /// The input recorded since the last call.
    pub fn take(&mut self) -> Vec<StrokeInput> {
        std::mem::take(&mut self.events)
    }

    /// True while a stroke gesture is held.
    pub fn is_down(&self) -> bool {
        self.down
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    fn input(events: Vec<Event>) -> egui::InputState {
        let mut i = egui::InputState::default();
        i.events = events;
        i
    }

    #[test]
    fn a_gesture_is_down_moves_and_up_in_view_coordinates() {
        let rect = Rect::from_min_size(pos2(100.0, 50.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        let mut first = input(vec![Event::PointerMoved(pos2(110.0, 60.0))]);
        first.pointer = Default::default();
        log.record(&first, rect, true, true, false, None);
        // without a press origin the gesture can't start
        assert!(log.take().is_empty() && !log.is_down());
        log.down = true;
        let moves = input(vec![Event::PointerMoved(pos2(120.0, 70.0)), Event::PointerMoved(pos2(130.0, 80.0))]);
        log.record(&moves, rect, true, false, false, None);
        let got = log.take();
        assert_eq!(got.len(), 2);
        assert!(matches!(got[1], StrokeInput::Move(s) if s.pos == [30.0, 30.0] && s.pressure == 1.0));
        log.record(&input(Vec::new()), rect, false, false, true, None);
        assert_eq!(log.take(), vec![StrokeInput::Up]);
        assert!(!log.is_down());
    }

    #[test]
    fn a_line_gesture_is_one_line_event_without_moves() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        // a started line gesture (egui's test input has no press origin to start one)
        log.events.push(StrokeInput::Line { to: PointerSample { pos: [40.0, 50.0], pressure: 1.0, time: 0.0 }, snap: true });
        log.down = true;
        log.line = true;
        log.record(&input(vec![Event::PointerMoved(pos2(60.0, 70.0))]), rect, true, false, false, Some(true));
        log.record(&input(Vec::new()), rect, false, false, true, Some(true));
        let got = log.take();
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(matches!(got[0], StrokeInput::Line { snap: true, .. }) && got[1] == StrokeInput::Up);
        assert!(!log.line && !log.is_down());
    }
}
