//! Turning the next input into a binding while the shortcuts page waits for one.

use eframe::egui::{Event, Key};

use super::chord::{Chord, Input, Mods};
use super::table::Kind;

/// What the input of one frame means while a binding is being recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Captured {
    /// Nothing usable happened; keep waiting.
    Nothing,
    /// Escape: stop without changing anything.
    Cancel,
    /// Backspace or Delete: remove the binding.
    Clear,
    Chord(Chord),
}

/// Reads `events` for an action of `kind`: keys for key actions, a pressed pointer button
/// (mouse or pen) or the wheel for gestures. The first usable event wins.
pub fn capture(events: &[Event], kind: Kind) -> Captured {
    for e in events {
        match e {
            Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if Mods::from(*modifiers) == Mods::NONE => return Captured::Cancel,
            Event::Key { key: Key::Backspace | Key::Delete, pressed: true, modifiers, .. } if Mods::from(*modifiers) == Mods::NONE => return Captured::Clear,
            Event::Key { key, pressed: true, modifiers, .. } if kind != Kind::Gesture => return Captured::Chord(Chord::key(Mods::from(*modifiers), *key)),
            Event::PointerButton { button, pressed: true, modifiers, .. } if kind == Kind::Gesture => {
                return Captured::Chord(Chord { mods: Mods::from(*modifiers), input: Input::Drag(*button) });
            }
            Event::MouseWheel { modifiers, .. } if kind == Kind::Gesture => {
                return Captured::Chord(Chord { mods: Mods::from(*modifiers), input: Input::Wheel });
            }
            _ => {}
        }
    }
    Captured::Nothing
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Modifiers, MouseWheelUnit, PointerButton, pos2, vec2};

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    #[test]
    fn key_actions_take_keys_with_their_modifiers() {
        let c = capture(&[key(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)], Kind::Press);
        assert_eq!(c, Captured::Chord(Chord::parse("Mod+Shift+Z").expect("chord")));
        assert_eq!(capture(&[key(Key::Escape, Modifiers::NONE)], Kind::Press), Captured::Cancel);
        assert_eq!(capture(&[key(Key::Backspace, Modifiers::NONE)], Kind::Hold), Captured::Clear);
        let click = Event::PointerButton { pos: pos2(0.0, 0.0), button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE };
        assert_eq!(capture(&[click], Kind::Press), Captured::Nothing);
    }

    #[test]
    fn gestures_take_mouse_and_pen_buttons_and_the_wheel() {
        let press = Event::PointerButton { pos: pos2(0.0, 0.0), button: PointerButton::Middle, pressed: true, modifiers: Modifiers::ALT };
        assert_eq!(capture(&[press], Kind::Gesture), Captured::Chord(Chord::parse("Alt+MiddleDrag").expect("chord")));
        let wheel = Event::MouseWheel { unit: MouseWheelUnit::Line, delta: vec2(0.0, 1.0), phase: eframe::egui::TouchPhase::Move, modifiers: Modifiers::SHIFT };
        assert_eq!(capture(&[wheel], Kind::Gesture), Captured::Chord(Chord::parse("Shift+Wheel").expect("chord")));
        assert_eq!(capture(&[key(Key::A, Modifiers::NONE)], Kind::Gesture), Captured::Nothing);
    }
}
