//! Input helpers egui no longer provides.

use eframe::egui::{self, Event, InputState, MouseWheelUnit, Vec2, vec2};

/// The unsmoothed wheel movement of this frame in points, as egui 0.29's `InputState::raw_scroll_delta`
/// reported it: lines and pages converted to points, Shift turning vertical scrolling into horizontal.
/// The editor's zoom controls use it so that one wheel notch zooms at once instead of over several frames.
pub fn raw_scroll_delta(i: &InputState) -> Vec2 {
    let line = egui::InputOptions::default().line_scroll_speed;
    let page = i.content_rect().height();
    let mut sum = Vec2::ZERO;
    for e in &i.raw.events {
        if let Event::MouseWheel { unit, delta, modifiers, .. } = e {
            let mut d = match unit {
                MouseWheelUnit::Point => *delta,
                MouseWheelUnit::Line => *delta * line,
                MouseWheelUnit::Page => *delta * page,
            };
            if modifiers.shift {
                d = vec2(d.x + d.y, 0.0);
            }
            sum += d;
        }
    }
    sum
}
