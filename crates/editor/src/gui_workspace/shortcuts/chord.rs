//! One binding: modifiers plus a key, a pointer-button drag or the wheel (WS-12).
//!
//! Ctrl on Windows and Linux and Cmd on macOS are one modifier, written `Mod` in the settings
//! file and shown as `Ctrl` or `Cmd` for the platform; the mapping happens here and nowhere else.

use std::fmt;

use eframe::egui::{Key, Modifiers, PointerButton};

/// The modifiers a binding needs, held exactly (no more, no fewer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Mods {
    /// Ctrl on Windows and Linux, Cmd on macOS.
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { command: false, shift: false, alt: false };
}

impl From<Modifiers> for Mods {
    fn from(m: Modifiers) -> Mods {
        Mods { command: m.command, shift: m.shift, alt: m.alt }
    }
}

/// What is pressed besides the modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    Key(Key),
    /// Dragging with a mouse or pen button held.
    Drag(PointerButton),
    /// The mouse wheel or a two-finger scroll.
    Wheel,
}

/// A complete binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chord {
    pub mods: Mods,
    pub input: Input,
}

impl Chord {
    pub fn key(mods: Mods, key: Key) -> Chord {
        Chord { mods, input: Input::Key(key) }
    }

    /// Parses the settings form, e.g. `Mod+Shift+Z`, `Alt+LeftDrag`, `F8`, `Wheel`. `Ctrl` and
    /// `Cmd` are read as `Mod`, so files written on another platform still work. Case is ignored.
    pub fn parse(text: &str) -> Option<Chord> {
        // "Ctrl++": the key is the plus sign.
        let text = match text.trim().strip_suffix("++") {
            Some(head) => format!("{head}+Plus"),
            None => text.trim().to_owned(),
        };
        let parts: Vec<&str> = text.split('+').map(str::trim).collect();
        let (last, mods) = parts.split_last()?;
        let mut m = Mods::NONE;
        for p in mods {
            match p.to_ascii_lowercase().as_str() {
                "mod" | "ctrl" | "cmd" | "command" | "control" => m.command = true,
                "shift" => m.shift = true,
                "alt" | "option" | "opt" => m.alt = true,
                _ => return None,
            }
        }
        let input = match last.to_ascii_lowercase().as_str() {
            "wheel" => Input::Wheel,
            "leftdrag" => Input::Drag(PointerButton::Primary),
            "rightdrag" => Input::Drag(PointerButton::Secondary),
            "middledrag" => Input::Drag(PointerButton::Middle),
            "backdrag" => Input::Drag(PointerButton::Extra1),
            "forwarddrag" => Input::Drag(PointerButton::Extra2),
            _ => Input::Key(key_from_name(last)?),
        };
        Some(Chord { mods: m, input })
    }

    /// The text shown in the UI, with the platform's name for the command modifier.
    pub fn label(&self) -> String {
        self.render(if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" })
    }

    fn render(&self, command: &str) -> String {
        let mut out = String::new();
        let mut push = |s: &str| {
            if !out.is_empty() {
                out.push('+');
            }
            out.push_str(s);
        };
        if self.mods.command {
            push(command);
        }
        if self.mods.alt {
            push(if cfg!(target_os = "macos") && command != "Mod" { "Option" } else { "Alt" });
        }
        if self.mods.shift {
            push("Shift");
        }
        push(match self.input {
            Input::Key(k) => k.name(),
            Input::Wheel => "Wheel",
            Input::Drag(PointerButton::Primary) => "LeftDrag",
            Input::Drag(PointerButton::Secondary) => "RightDrag",
            Input::Drag(PointerButton::Middle) => "MiddleDrag",
            Input::Drag(PointerButton::Extra1) => "BackDrag",
            Input::Drag(PointerButton::Extra2) => "ForwardDrag",
        });
        out
    }
}

/// The settings form: the command modifier is always `Mod`.
impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render("Mod"))
    }
}

/// egui's key names, ignoring case.
fn key_from_name(name: &str) -> Option<Key> {
    Key::from_name(name).or_else(|| Key::ALL.iter().copied().find(|k| k.name().eq_ignore_ascii_case(name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_form_round_trips_and_ctrl_and_cmd_are_one_modifier() {
        for text in ["Mod+Shift+Z", "Alt+LeftDrag", "Alt+Shift+LeftDrag", "F8", "4", "Wheel", "Shift+RightDrag", "Mod+Plus", "Alt+MiddleDrag"] {
            let c = Chord::parse(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(c.to_string(), text);
        }
        assert_eq!(Chord::parse("Ctrl+O"), Chord::parse("Mod+O"));
        assert_eq!(Chord::parse("cmd+o"), Chord::parse("Mod+O"));
        assert_eq!(Chord::parse("Mod++"), Some(Chord::key(Mods { command: true, ..Mods::NONE }, Key::Plus)));
        assert_eq!(Chord::parse("4"), Some(Chord::key(Mods::NONE, Key::Num4)));
    }

    #[test]
    fn nonsense_is_refused() {
        for text in ["", "+", "Hyper+A", "Mod+", "Alt+Drag", "NoSuchKey"] {
            assert_eq!(Chord::parse(text), None, "{text}");
        }
    }

    #[test]
    fn the_label_uses_the_platform_name_once() {
        let c = Chord::parse("Mod+Shift+Z").expect("chord");
        let want = if cfg!(target_os = "macos") { "Cmd+Shift+Z" } else { "Ctrl+Shift+Z" };
        assert_eq!(c.label(), want);
        let m = Modifiers { ctrl: true, command: !cfg!(target_os = "macos"), mac_cmd: false, ..Default::default() };
        assert_eq!(Mods::from(m).command, !cfg!(target_os = "macos"));
    }
}
