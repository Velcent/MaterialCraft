//! Pointer input for the brush tools (PT-56): the viewport records each frame's stroke gesture as
//! [`StrokeInput`]s (every pointer move egui saw, not only the frame's last position) with pen pressure where the
//! platform reports it (touch force; a mouse is full pressure). Tablet tilt and rotation come with roadmap step 35.

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
}

/// The stroke gesture's input since the tools last took it.
#[derive(Default)]
pub struct PointerLog {
    events: Vec<StrokeInput>,
    down: bool,
    pressure: f32,
}

impl PointerLog {
    /// Records this frame's input of the view at `rect`; `stroking` says whether the stroke gesture is held
    /// (pressed in the view with the stroke binding), `started` / `stopped` whether it began or ended this frame.
    pub fn record(&mut self, input: &egui::InputState, rect: Rect, stroking: bool, started: bool, stopped: bool) {
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
                self.events.push(StrokeInput::Down(local(p)));
                self.down = true;
            }
        }
        if self.down && (stroking || stopped) {
            for e in &input.events {
                if let Event::PointerMoved(p) = e {
                    self.events.push(StrokeInput::Move(local(*p)));
                }
            }
        }
        if self.down && (stopped || !stroking) {
            self.events.push(StrokeInput::Up);
            self.down = false;
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
        log.record(&first, rect, true, true, false);
        // without a press origin the gesture can't start
        assert!(log.take().is_empty() && !log.is_down());
        log.down = true;
        let moves = input(vec![Event::PointerMoved(pos2(120.0, 70.0)), Event::PointerMoved(pos2(130.0, 80.0))]);
        log.record(&moves, rect, true, false, false);
        let got = log.take();
        assert_eq!(got.len(), 2);
        assert!(matches!(got[1], StrokeInput::Move(s) if s.pos == [30.0, 30.0] && s.pressure == 1.0));
        log.record(&input(Vec::new()), rect, false, false, true);
        assert_eq!(log.take(), vec![StrokeInput::Up]);
        assert!(!log.is_down());
    }
}
