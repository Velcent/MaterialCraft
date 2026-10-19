//! Pointer input for the brush tools (PT-56): the viewport records each frame's stroke gesture as
//! [`StrokeInput`]s (every pointer move egui saw, not only the frame's last position), each with the pen's
//! pressure, tilt and barrel rotation merged in from the [`Stylus`] (tablet samples from [`tablet`], touch force on
//! Windows; a mouse is full pressure, no tilt). The pressure has been through the user's pressure curve
//! ([`pressure_curve`]) before any brush sees it. A straight-line gesture (PT-31) gives one [`StrokeInput::Line`] at
//! the click and ignores the drag after it.

pub mod pen_status;
pub mod pressure_curve;
pub mod stylus;
pub mod tablet;
pub mod tablet_probe;
pub mod window_system;

use eframe::egui::{self, Event, Pos2, Rect};
use stylus::{Stylus, StylusFeed};

use crate::gui_settings::PenSettings;

/// One pointer sample in view coordinates (UI points from the view's top-left corner).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerSample {
    pub pos: [f32; 2],
    /// 0 … 1; 1 for a mouse.
    pub pressure: f32,
    /// Pen tilt in degrees, x and y (−90 … 90; 0 for a mouse).
    pub tilt: [f32; 2],
    /// Pen barrel rotation in degrees (0 … 360; 0 for a mouse).
    pub rotation: f32,
    /// Seconds (egui's input clock).
    pub time: f64,
}

impl PointerSample {
    /// A mouse sample: full pressure, no tilt or rotation.
    pub fn mouse(pos: [f32; 2], time: f64) -> PointerSample {
        PointerSample { pos, pressure: 1.0, tilt: [0.0; 2], rotation: 0.0, time }
    }
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
    /// The pen state merged into every sample.
    stylus: Stylus,
}

impl PointerLog {
    /// Records this frame's input of the view at `rect`; `stroking` says whether the stroke gesture is held
    /// (pressed in the view with the stroke binding), `started` / `stopped` whether it began or ended this frame,
    /// `line` whether it is a straight-line gesture (and whether that snaps).
    pub fn record(&mut self, input: &egui::InputState, rect: Rect, stroking: bool, started: bool, stopped: bool, line: Option<bool>) {
        self.stylus.update(&input.events);
        let (time, origin) = (input.time, rect.min);
        if started && !self.down {
            let at = input.pointer.press_origin().or(input.pointer.interact_pos());
            if let Some(p) = at {
                self.stylus.begin_stroke();
                // a stroke pressed and released in one frame is a tap: it paints at the pen's peak
                let s = self.sample(p, origin, time, started && stopped);
                self.events.push(match line {
                    Some(snap) => StrokeInput::Line { to: s, snap },
                    None => StrokeInput::Down(s),
                });
                self.down = true;
                self.line = line.is_some();
            }
        }
        if self.down && !self.line && (stroking || stopped) {
            let n = input.events.iter().filter(|e| matches!(e, Event::PointerMoved(_))).count();
            let moves = input.events.iter().filter_map(|e| if let Event::PointerMoved(p) = e { Some(*p) } else { None });
            for (k, p) in moves.enumerate() {
                self.stylus.select(k, n);
                let s = self.sample(p, origin, time, false);
                self.events.push(StrokeInput::Move(s));
            }
            self.stylus.clear_selection();
        }
        if self.down && (stopped || !stroking) {
            self.events.push(StrokeInput::Up);
            self.down = false;
            self.line = false;
        }
    }

    /// The sample at `p` with the stylus's current pen state (a mouse's without a pen).
    fn sample(&self, p: Pos2, origin: Pos2, time: f64, tap: bool) -> PointerSample {
        let pos = [p.x - origin.x, p.y - origin.y];
        match self.stylus.sample() {
            Some(pen) => PointerSample {
                pos,
                pressure: if tap { self.stylus.tap_pressure() } else { pen.pressure },
                tilt: [pen.tilt_x, pen.tilt_y],
                rotation: pen.rotation,
                time,
            },
            None => PointerSample::mouse(pos, time),
        }
    }

    /// Applies the user's pen settings (whether pressure is used, the pressure curve).
    pub fn configure(&mut self, pen: &PenSettings) {
        self.stylus.use_pressure = pen.use_pressure;
        self.stylus.set_pressure_curve(&pen.pressure_curve);
    }

    /// Where the platform's pen reader writes its samples.
    pub fn feed(&self) -> StylusFeed {
        self.stylus.feed.clone()
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
    use super::stylus::PenSample;
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
        assert!(matches!(got[1], StrokeInput::Move(s) if s.pos == [30.0, 30.0] && s.pressure == 1.0 && s.tilt == [0.0; 2]));
        log.record(&input(Vec::new()), rect, false, false, true, None);
        assert_eq!(log.take(), vec![StrokeInput::Up]);
        assert!(!log.is_down());
    }

    #[test]
    fn a_line_gesture_is_one_line_event_without_moves() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        // a started line gesture (egui's test input has no press origin to start one)
        log.events.push(StrokeInput::Line { to: PointerSample::mouse([40.0, 50.0], 0.0), snap: true });
        log.down = true;
        log.line = true;
        log.record(&input(vec![Event::PointerMoved(pos2(60.0, 70.0))]), rect, true, false, false, Some(true));
        log.record(&input(Vec::new()), rect, false, false, true, Some(true));
        let got = log.take();
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(matches!(got[0], StrokeInput::Line { snap: true, .. }) && got[1] == StrokeInput::Up);
        assert!(!log.line && !log.is_down());
    }

    /// The Move samples of `got`.
    fn moves(got: &[StrokeInput]) -> Vec<PointerSample> {
        got.iter().filter_map(|e| if let StrokeInput::Move(s) = e { Some(*s) } else { None }).collect()
    }

    fn pen(pressure: f32, tilt_x: f32, rotation: f32) -> Option<PenSample> {
        Some(PenSample { pressure, tilt_x, tilt_y: -tilt_x / 2.0, rotation, eraser: false })
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn tablet_samples_merge_into_each_pointer_move() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        let feed = log.feed();
        // the pen touches down at 0.2 (the press frame), then reports 0.4 / 0.6 / 0.8 while four moves arrive
        feed.set(pen(0.2, 10.0, 350.0));
        log.record(&input(Vec::new()), rect, true, false, false, None);
        log.stylus.begin_stroke();
        let down = log.sample(pos2(5.0, 5.0), rect.min, 0.0, false);
        assert_eq!((down.pressure, down.tilt, down.rotation), (0.2, [10.0, -5.0], 350.0));
        log.down = true;
        for (p, r) in [(0.4, 10.0), (0.6, 30.0), (0.8, 50.0)] {
            feed.set(pen(p, 20.0, r));
        }
        let ev: Vec<Event> = (0..4).map(|i| Event::PointerMoved(pos2(10.0 + i as f32, 10.0))).collect();
        log.record(&input(ev), rect, true, false, false, None);
        let got = moves(&log.take());
        let pressures: Vec<f32> = got.iter().map(|s| s.pressure).collect();
        let want = [0.35, 0.5, 0.65, 0.8];
        assert!(pressures.iter().zip(want).all(|(g, w)| close(*g, w)), "{pressures:?}");
        // tilt eases from 10° to 20°, rotation from 350° through 0° to 50°
        assert!(close(got[0].tilt[0], 17.5) && close(got[3].tilt[0], 20.0) && close(got[3].tilt[1], -10.0), "{got:?}");
        assert!(close(got[0].rotation, 5.0) && close(got[3].rotation, 50.0), "{got:?}");
        // the next frame without new pen samples keeps the pen's current state
        log.record(&input(vec![Event::PointerMoved(pos2(20.0, 10.0))]), rect, true, false, false, None);
        assert!(moves(&log.take()).iter().all(|s| close(s.pressure, 0.8) && close(s.rotation, 50.0)));
    }

    #[test]
    fn the_pressure_curve_applies_before_the_samples_leave() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        log.configure(&PenSettings { use_pressure: true, pressure_curve: vec![[0.0, 0.0], [0.5, 0.25], [1.0, 1.0]], ..PenSettings::default() });
        log.feed().set(pen(0.5, 0.0, 0.0));
        log.down = true;
        log.record(&input(vec![Event::PointerMoved(pos2(1.0, 1.0))]), rect, true, false, false, None);
        assert!(moves(&log.take()).iter().all(|s| close(s.pressure, 0.25)));
        // pressure off: the pen paints like a mouse
        log.configure(&PenSettings { use_pressure: false, ..PenSettings::default() });
        log.feed().set(pen(0.5, 30.0, 90.0));
        log.record(&input(vec![Event::PointerMoved(pos2(2.0, 1.0))]), rect, true, false, false, None);
        assert_eq!(moves(&log.take()), vec![PointerSample::mouse([2.0, 1.0], 0.0)]);
    }

    #[test]
    fn a_mouse_after_the_pen_is_full_pressure_again() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        log.configure(&PenSettings { use_pressure: true, pressure_curve: vec![[0.0, 0.0], [1.0, 0.1]], ..PenSettings::default() });
        let feed = log.feed();
        feed.set(pen(0.3, 5.0, 5.0));
        log.down = true;
        log.record(&input(vec![Event::PointerMoved(pos2(1.0, 1.0))]), rect, true, false, false, None);
        assert!(moves(&log.take()).iter().all(|s| s.pressure < 0.1));
        // the X11 reader reports `None` when another pointer moves: a mouse, whatever the curve
        feed.set(None);
        log.record(&input(Vec::new()), rect, true, false, false, None);
        log.record(&input(vec![Event::PointerMoved(pos2(3.0, 1.0))]), rect, true, false, false, None);
        assert_eq!(moves(&log.take()), vec![PointerSample::mouse([3.0, 1.0], 0.0)]);
    }

    #[test]
    fn windows_touch_force_is_pressure_without_tilt() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0));
        let mut log = PointerLog::default();
        log.down = true;
        let touch =
            Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase: egui::TouchPhase::Move, pos: pos2(1.0, 1.0), force: Some(0.45) };
        log.record(&input(vec![touch, Event::PointerMoved(pos2(1.0, 1.0))]), rect, true, false, false, None);
        let got = moves(&log.take());
        assert!(got.len() == 1 && close(got[0].pressure, 0.45) && got[0].tilt == [0.0; 2], "{got:?}");
    }
}
