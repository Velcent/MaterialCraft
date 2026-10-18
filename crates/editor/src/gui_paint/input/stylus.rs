// The pen sample feed and its merge with egui's pointer (`PenSample`, `StylusFeed`, `Stylus`: the feed queue, touch
// force tracking, per-move interpolation of the frame's pen samples, the pressure curve and the tap pressure) follow
// PhotoCraft's crates/ui-egui/src/stylus.rs, revision 5f65579, with PhotoCraft's app parts left out (the eraser-end
// tool switch, the journal's stroke points, touch press-and-hold) and its pressure curve taken from the brush crate.
// See ATTRIBUTION.md.
//
// Copyright (c) 2026 ArtCraft Team and the PhotoCraft contributors.
// Licensed under either of the Apache License, Version 2.0 or the MIT license, at your option.

//! Stylus input: pen pressure, tilt and barrel rotation where the platform reports them. A mouse paints at
//! pressure 1 with no tilt.
//!
//! Sources, by platform (eframe 0.36, winit 0.30):
//!
//! - **Linux X11** (Xwayland included): `photocraft-tablet` reads XInput2 raw valuator events on its own X
//!   connection and writes them into the [`StylusFeed`] ([`super::tablet`]).
//! - **Linux Wayland**: compositors give a pen only to clients that bind `zwp_tablet_v2`, which winit doesn't, so
//!   a native Wayland window gets no pen samples; the window has to run through Xwayland for them.
//! - **macOS**: winit drops `NSEvent` tablet data, so `photocraft-tablet`'s AppKit event monitor writes pressure,
//!   tilt, rotation and the eraser end into the feed.
//! - **Windows**: winit turns `WM_POINTER` pen input into `egui::Event::Touch` with a force, which
//!   [`Stylus::update`] reads. winit drops tilt and rotation there (they stay 0), and reports a pen pressed below
//!   its first level with no force, which reads as pressure 0 once a pen is known.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui;
use materialcraft_brush::ResponseCurve;

/// One stylus reading. Tilt is in degrees (−90 … 90, W3C Pointer Events convention), rotation in degrees 0 … 360.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PenSample {
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
    pub rotation: f32,
    /// The pen's eraser end is in use.
    pub eraser: bool,
}

impl Default for PenSample {
    fn default() -> Self {
        Self { pressure: 1.0, tilt_x: 0.0, tilt_y: 0.0, rotation: 0.0, eraser: false }
    }
}

impl PenSample {
    /// Clamp to the documented ranges (and replace non-finite values).
    pub fn sanitized(self) -> Self {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        Self {
            pressure: f(self.pressure, 0.0, 1.0, 1.0),
            tilt_x: f(self.tilt_x, -90.0, 90.0, 0.0),
            tilt_y: f(self.tilt_y, -90.0, 90.0, 0.0),
            // `rem_euclid` rounds tiny negative angles up to exactly 360.
            rotation: if self.rotation.is_finite() { self.rotation.rem_euclid(360.0) % 360.0 } else { 0.0 },
            eraser: self.eraser,
        }
    }
}

/// Most samples a [`StylusFeed`] keeps between two frames (a stalled UI drops the oldest).
const FEED_QUEUE: usize = 1024;

#[derive(Debug, Default)]
struct FeedState {
    /// The current sample (`None` = no pen).
    latest: Option<PenSample>,
    /// Every sample set since the view last drained them, oldest first.
    queue: VecDeque<PenSample>,
}

/// Shared state a platform reader writes pen samples into (from its own thread). Cloning shares it. Each sample
/// both becomes the current one and is queued, so a frame sees every sample the pen reported, in order, not just
/// the last one (the pressure of each pointer move is interpolated from them).
#[derive(Clone, Debug, Default)]
pub struct StylusFeed(Arc<Mutex<FeedState>>);

impl StylusFeed {
    /// The current sample (`None` = a mouse again); a sample is also queued for [`Self::drain`].
    pub fn set(&self, s: Option<PenSample>) {
        let mut g = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let s = s.map(PenSample::sanitized);
        g.latest = s;
        if let Some(s) = s {
            if g.queue.len() >= FEED_QUEUE {
                g.queue.pop_front();
            }
            g.queue.push_back(s);
        }
    }

    pub fn get(&self) -> Option<PenSample> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).latest
    }

    /// The samples set since the last drain, oldest first.
    pub fn drain(&self) -> Vec<PenSample> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).queue.drain(..).collect()
    }
}

/// The view's stylus state: the feed, egui's touch forces and the pressure curve, merged into one pen sample per
/// pointer move.
#[derive(Clone, Debug)]
pub struct Stylus {
    /// Pen samples pushed by the platform reader.
    pub feed: StylusFeed,
    /// Off: a pen paints like a mouse.
    pub use_pressure: bool,
    /// The pressure curve (`None`: linear), with the points it was built from.
    pressure_curve: Option<ResponseCurve>,
    curve_points: Vec<[f32; 2]>,
    /// Force of the touch/pen contact currently down (egui `Event::Touch`).
    touch: Option<f32>,
    /// The touch contacts currently down, so a touch move outside them is a hovering pen.
    contacts: Vec<egui::TouchId>,
    /// A pen reported through touch events (it hovered, or a contact carried a force): its contacts without a
    /// force are at pressure 0, not a finger's full pressure.
    touch_pen: bool,
    /// The contact ended this frame: keep its force for this frame's last moves, clear next frame.
    lifted: bool,
    /// This frame's pen samples, oldest first (the feed's queue and the touch forces).
    frame: Vec<PenSample>,
    /// The last pen sample of an earlier frame: where this frame's interpolation starts.
    prev: Option<PenSample>,
    /// The sample [`Self::select`] interpolated for the pointer move being processed.
    current: Option<PenSample>,
    /// The highest pen pressure (before the curve) since the stroke began, for a tap's one dab.
    peak: f32,
}

impl Default for Stylus {
    fn default() -> Self {
        Self {
            feed: StylusFeed::default(),
            use_pressure: true,
            pressure_curve: None,
            curve_points: Vec::new(),
            touch: None,
            contacts: Vec::new(),
            touch_pen: false,
            lifted: false,
            frame: Vec::new(),
            prev: None,
            current: None,
            peak: 0.0,
        }
    }
}

impl Stylus {
    /// Once per frame: take the feed's samples and track touch/pen contacts from this frame's input events.
    pub fn update(&mut self, events: &[egui::Event]) {
        if std::mem::take(&mut self.lifted) {
            self.touch = None;
        }
        if let Some(last) = self.frame.last() {
            self.prev = Some(*last);
        }
        self.frame = self.feed.drain();
        self.current = None;
        for e in events {
            if let egui::Event::Touch { id, phase, force, .. } = e {
                let force = force.filter(|f| f.is_finite()).map(|f| f.clamp(0.0, 1.0));
                match phase {
                    egui::TouchPhase::Start | egui::TouchPhase::Move => {
                        let start = *phase == egui::TouchPhase::Start;
                        if start {
                            // A new contact keeps its force into the next frame, even when an earlier contact
                            // lifted in this one.
                            self.lifted = false;
                            if !self.contacts.contains(id) {
                                if self.contacts.len() >= 16 {
                                    self.contacts.remove(0);
                                }
                                self.contacts.push(*id);
                            }
                        }
                        let down = start || self.contacts.contains(id);
                        // Only a pen moves without touching, and only a pen (or a force-sensing screen) reports a
                        // force.
                        if !down || force.is_some() {
                            self.touch_pen = true;
                        }
                        // Windows reports a pen pressed lighter than its first pressure level with no force at all
                        // (winit maps pressure 0 to `None`): a light tap is pressure 0, never a mouse click's full
                        // pressure. A finger on a screen without force sensing stays at full pressure.
                        let force = force.or((down && self.touch_pen && (start || self.touch.is_none())).then_some(0.0));
                        if let Some(f) = force {
                            self.touch = Some(f);
                            self.frame.push(PenSample { pressure: f, ..Default::default() });
                        }
                    }
                    egui::TouchPhase::End | egui::TouchPhase::Cancel => {
                        self.lifted = true;
                        self.contacts.retain(|c| c != id);
                    }
                }
            }
        }
        // A pen reports pressure 0 while it hovers and as it lifts (X11, macOS): that is no contact, so a frame
        // that also touched keeps only its contact samples. A stroke's last moves then keep the pressure the pen
        // left with, and a tap its own.
        if self.frame.iter().any(|s| s.pressure > 0.0) {
            self.frame.retain(|s| s.pressure > 0.0);
        }
        self.peak = self.frame.iter().fold(self.peak, |m, s| m.max(s.pressure));
    }

    /// The pen's current sample. When the pen touched during this frame but has lifted since (the feed is cleared
    /// or back at pressure 0 by the time the UI runs, as for a tap shorter than a frame), the frame's last contact
    /// sample: never full pressure as for a mouse, and never the lift's 0.
    fn latest(&self) -> Option<PenSample> {
        let feed = self.feed.get();
        match self.frame.last() {
            Some(s) if s.pressure > 0.0 && feed.is_none_or(|f| f.pressure <= 0.0) => Some(*s),
            _ => feed,
        }
    }

    /// The current pen sample through the pressure curve, `None` for a mouse (and for any pen while pressure is
    /// off).
    pub fn sample(&self) -> Option<PenSample> {
        if !self.use_pressure {
            return None;
        }
        let s = self.current.or_else(|| self.latest()).or(self.touch.map(|pressure| PenSample { pressure, ..Default::default() }))?;
        // The pen's pressure through the user's curve, before any brush sees it.
        Some(match &self.pressure_curve {
            Some(c) => PenSample { pressure: c.eval(s.pressure), ..s },
            None => s,
        })
    }

    /// The pressure curve's points (rebuilt only when they change).
    pub fn set_pressure_curve(&mut self, points: &[[f32; 2]]) {
        if self.curve_points != points {
            self.curve_points = points.to_vec();
            let c = ResponseCurve::new(points);
            self.pressure_curve = (!c.is_linear()).then_some(c);
        }
    }

    /// Choose the pen sample for move `k` of the `n` pointer moves this frame delivered: the pen reports at its own
    /// rate, so each move takes the samples interpolated at its share of the frame (from the previous frame's last
    /// sample), instead of all sharing the latest one. A mouse, or a frame without pen samples, keeps the current
    /// sample.
    pub fn select(&mut self, k: usize, n: usize) {
        self.current = None;
        if !self.use_pressure || n == 0 || self.frame.is_empty() || self.sample().is_none() {
            return;
        }
        let m = self.frame.len();
        let start = self.prev.or(self.frame.first().copied()).unwrap_or_default();
        let at = |i: usize| if i == 0 { start } else { self.frame.get(i - 1).copied().unwrap_or(start) };
        let pos = (k + 1).min(n) as f32 / n as f32 * m as f32;
        let i = pos.floor() as usize;
        let (a, b, t) = (at(i), at((i + 1).min(m)), pos - pos.floor());
        let lerp = |x: f32, y: f32| x + (y - x) * t;
        // Rotation takes the shorter way round.
        let dr = (b.rotation - a.rotation + 540.0).rem_euclid(360.0) - 180.0;
        self.current = Some(
            PenSample {
                pressure: lerp(a.pressure, b.pressure),
                tilt_x: lerp(a.tilt_x, b.tilt_x),
                tilt_y: lerp(a.tilt_y, b.tilt_y),
                rotation: a.rotation + dr * t,
                eraser: b.eraser,
            }
            .sanitized(),
        );
    }

    /// Back to the current sample after a frame's moves.
    pub fn clear_selection(&mut self) {
        self.current = None;
    }

    /// Pressure for the next move (1 for a mouse).
    #[cfg(test)]
    pub fn pressure(&self) -> f32 {
        self.sample().map_or(1.0, |s| s.pressure)
    }

    /// Pressure for a tap, a stroke released in the frame it was pressed: the highest the pen pressed since the
    /// stroke began (through the pressure curve), as the first sample of a contact is often far lighter than the
    /// tap. 1 for a mouse.
    pub fn tap_pressure(&self) -> f32 {
        let Some(s) = self.sample() else { return 1.0 };
        let peak = match &self.pressure_curve {
            Some(c) => c.eval(self.peak),
            None => self.peak,
        };
        if self.peak > 0.0 { peak.max(s.pressure) } else { s.pressure }
    }

    /// A stroke begins: its first moves interpolate from this frame's samples, never from a sample left over from
    /// an earlier stroke.
    pub fn begin_stroke(&mut self) {
        self.prev = None;
        self.peak = self.frame.iter().fold(0.0, |m, s| m.max(s.pressure));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(phase: egui::TouchPhase, force: Option<f32>) -> egui::Event {
        touch_id(0, phase, force)
    }

    fn touch_id(id: u64, phase: egui::TouchPhase, force: Option<f32>) -> egui::Event {
        egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos: egui::pos2(1.0, 1.0), force }
    }

    fn pen(pressure: f32) -> PenSample {
        PenSample { pressure, ..Default::default() }
    }

    #[test]
    fn the_feed_queues_every_sample_in_order_and_keeps_the_latest() {
        let feed = StylusFeed::default();
        for p in [0.1, 0.2, 0.3] {
            feed.set(Some(pen(p)));
        }
        assert_eq!(feed.get().map(|s| s.pressure), Some(0.3));
        assert_eq!(feed.drain().iter().map(|s| s.pressure).collect::<Vec<_>>(), vec![0.1, 0.2, 0.3]);
        assert!(feed.drain().is_empty(), "drained once");
        feed.set(None);
        assert_eq!(feed.get(), None, "a lifted pen is not queued");
        assert!(feed.drain().is_empty());
        // A UI that stops draining keeps only the newest samples.
        for i in 0..FEED_QUEUE + 10 {
            feed.set(Some(pen((i % 100) as f32 / 100.0)));
        }
        assert_eq!(feed.drain().len(), FEED_QUEUE);
    }

    #[test]
    fn each_move_takes_its_share_of_the_frames_pen_samples() {
        let mut s = Stylus::default();
        s.feed.set(Some(pen(0.2)));
        s.update(&[]);
        s.begin_stroke();
        // Next frame: the pen reported 0.4, 0.6 and 0.8 while six pointer moves arrived.
        for p in [0.4, 0.6, 0.8] {
            s.feed.set(Some(pen(p)));
        }
        s.update(&[]);
        let got: Vec<f32> = (0..6)
            .map(|k| {
                s.select(k, 6);
                s.pressure()
            })
            .collect();
        let want = [0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-6), "{got:?}");
        s.clear_selection();
        assert_eq!(s.pressure(), 0.8, "back to the current sample");
    }

    #[test]
    fn the_pressure_curve_reshapes_pen_pressure_but_not_the_mouse() {
        let mut s = Stylus::default();
        s.set_pressure_curve(&[[0.0, 0.0], [0.3, 0.6], [1.0, 1.0]]);
        s.feed.set(Some(pen(0.3)));
        s.update(&[]);
        assert!((s.pressure() - 0.6).abs() < 1e-5, "{}", s.pressure());
        s.set_pressure_curve(&[[0.0, 0.0], [1.0, 1.0]]);
        assert!((s.pressure() - 0.3).abs() < 1e-6, "linear again");
        // A mouse is full pressure whatever the curve.
        let mut m = Stylus::default();
        m.set_pressure_curve(&[[0.0, 0.0], [1.0, 0.2]]);
        m.update(&[egui::Event::PointerMoved(egui::pos2(1.0, 1.0))]);
        assert_eq!(m.pressure(), 1.0);
    }

    #[test]
    fn a_new_stroke_never_starts_from_an_old_strokes_pressure() {
        let mut s = Stylus::default();
        s.feed.set(Some(pen(0.9)));
        s.update(&[]);
        s.feed.set(Some(pen(0.1)));
        s.update(&[]);
        s.begin_stroke();
        s.select(0, 2);
        assert!((s.pressure() - 0.1).abs() < 1e-6, "{}", s.pressure());
    }

    #[test]
    fn a_mouse_or_pressure_off_paints_at_full_pressure() {
        let mut s = Stylus::default();
        s.update(&[egui::Event::PointerMoved(egui::pos2(1.0, 1.0))]);
        s.select(0, 1);
        assert_eq!((s.pressure(), s.sample()), (1.0, None));
        s.feed.set(Some(pen(0.3)));
        s.use_pressure = false;
        s.update(&[]);
        s.select(0, 1);
        assert_eq!((s.pressure(), s.sample()), (1.0, None));
    }

    #[test]
    fn rotation_between_samples_takes_the_short_way_round() {
        let mut s = Stylus::default();
        s.feed.set(Some(PenSample { rotation: 350.0, ..pen(1.0) }));
        s.update(&[]);
        s.feed.set(Some(PenSample { rotation: 10.0, ..pen(1.0) }));
        s.update(&[]);
        s.select(0, 2);
        let r = s.sample().map_or(-1.0, |p| p.rotation);
        assert!(r.abs() < 1e-3 || (r - 360.0).abs() < 1e-3, "halfway from 350° to 10° is 0°, got {r}");
    }

    #[test]
    fn touch_force_drives_pressure_until_lift() {
        let mut s = Stylus::default();
        s.update(&[touch(egui::TouchPhase::Start, Some(0.25))]);
        assert_eq!(s.pressure(), 0.25);
        s.update(&[touch(egui::TouchPhase::Move, Some(0.8)), touch(egui::TouchPhase::Move, Some(1.7))]);
        assert_eq!(s.pressure(), 1.0, "clamped");
        s.update(&[touch(egui::TouchPhase::Move, Some(f32::NAN))]);
        assert_eq!(s.pressure(), 1.0);
        s.update(&[touch(egui::TouchPhase::Move, Some(0.4))]);
        assert_eq!(s.pressure(), 0.4);
        // The lift frame still paints at the last force; the next frame is back to the mouse.
        s.update(&[touch(egui::TouchPhase::End, None)]);
        assert_eq!(s.pressure(), 0.4);
        s.update(&[]);
        assert_eq!(s.pressure(), 1.0);
    }

    #[test]
    fn a_light_windows_pen_tap_is_not_full_pressure() {
        use egui::TouchPhase::{End, Move, Start};
        // winit on Windows reports a pen pressed below its first pressure level with no force. The pen hovered
        // first, so the tap is pressure 0, not a mouse click's 1.
        let mut s = Stylus::default();
        s.update(&[touch_id(5, Move, None)]);
        assert_eq!(s.pressure(), 1.0, "hovering is no contact");
        s.update(&[touch_id(5, Start, None), touch_id(5, End, None)]);
        assert_eq!((s.pressure(), s.tap_pressure()), (0.0, 0.0));
        s.update(&[]);
        assert_eq!(s.pressure(), 1.0, "lifted");
        // A force-less move mid-stroke keeps the last force.
        s.update(&[touch_id(8, Start, Some(0.3))]);
        s.update(&[touch_id(8, Move, None)]);
        assert_eq!(s.pressure(), 0.3);
    }

    #[test]
    fn a_finger_without_force_still_paints_at_full_pressure() {
        use egui::TouchPhase::{End, Move, Start};
        let mut s = Stylus::default();
        s.update(&[touch_id(1, Start, None), touch_id(1, Move, None)]);
        assert_eq!(s.pressure(), 1.0);
        s.update(&[touch_id(1, End, None)]);
        s.update(&[]);
        assert_eq!(s.pressure(), 1.0);
    }

    #[test]
    fn the_feed_wins_over_touch_and_is_sanitized() {
        let mut s = Stylus::default();
        s.update(&[touch(egui::TouchPhase::Start, Some(0.3))]);
        s.feed.set(Some(PenSample { pressure: 0.6, tilt_x: 120.0, tilt_y: -30.0, rotation: -90.0, eraser: false }));
        assert_eq!(s.sample(), Some(PenSample { pressure: 0.6, tilt_x: 90.0, tilt_y: -30.0, rotation: 270.0, eraser: false }));
        s.feed.set(None);
        assert_eq!(s.pressure(), 0.3);
    }

    #[test]
    fn a_tap_paints_at_its_peak_pressure() {
        let mut s = Stylus::default();
        for p in [0.1, 0.7, 0.4] {
            s.feed.set(Some(pen(p)));
        }
        s.feed.set(Some(pen(0.0)));
        s.update(&[]);
        s.begin_stroke();
        assert!((s.tap_pressure() - 0.7).abs() < 1e-6, "{}", s.tap_pressure());
    }
}
