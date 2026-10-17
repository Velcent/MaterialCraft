//! The rebindable shortcut table: every action of a screen with its default bindings and the
//! user's changes (WS-12, PT-59). Screens declare their actions as [`ActionSpec`]s; the table
//! matches input against them and stores only what differs from the defaults.

use std::collections::BTreeMap;

use eframe::egui::{Event, InputState, Modifiers, PointerButton};

use super::chord::{Chord, Input, Mods};

/// How an action is triggered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A key press runs it once.
    Press,
    /// It is active while a key is held (a temporary mode).
    Hold,
    /// A pointer gesture: dragging with a button, or the wheel.
    Gesture,
}

impl Kind {
    /// Whether a chord can trigger an action of this kind.
    pub fn accepts(self, chord: &Chord) -> bool {
        match self {
            Kind::Press | Kind::Hold => matches!(chord.input, Input::Key(_)),
            Kind::Gesture => matches!(chord.input, Input::Drag(_) | Input::Wheel),
        }
    }
}

/// One action a screen offers, with its default bindings in settings form.
#[derive(Clone, Copy, Debug)]
pub struct ActionSpec {
    /// Stable id, used in the settings file (`paint.tool.eraser`).
    pub id: &'static str,
    pub label: &'static str,
    /// Heading on the shortcuts page.
    pub group: &'static str,
    pub kind: Kind,
    pub defaults: &'static [&'static str],
}

/// The bindings in force for a list of actions.
pub struct ShortcutTable {
    specs: &'static [ActionSpec],
    /// Parallel to `specs`.
    bindings: Vec<Vec<Chord>>,
}

impl ShortcutTable {
    /// The table with the user's `overrides` (action id → bindings) applied. Unknown ids and
    /// bindings that don't parse or don't suit the action are skipped and reported.
    pub fn new(specs: &'static [ActionSpec], overrides: &BTreeMap<String, Vec<String>>) -> (ShortcutTable, Vec<String>) {
        let mut problems = Vec::new();
        let bindings = specs
            .iter()
            .map(|s| match overrides.get(s.id) {
                Some(texts) => parse_all(s, texts, &mut problems),
                None => parse_all(s, s.defaults, &mut problems),
            })
            .collect();
        for id in overrides.keys().filter(|id| !specs.iter().any(|s| s.id == id.as_str())) {
            problems.push(format!("Unknown shortcut action '{id}' in the settings was ignored."));
        }
        (ShortcutTable { specs, bindings }, problems)
    }

    pub fn specs(&self) -> &'static [ActionSpec] {
        self.specs
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.specs.iter().position(|s| s.id == id)
    }

    /// The bindings of `id` (empty if unbound or unknown).
    pub fn bindings(&self, id: &str) -> &[Chord] {
        self.index(id).and_then(|i| self.bindings.get(i)).map_or(&[], Vec::as_slice)
    }

    /// The bindings of `id` for display, e.g. `Ctrl+O`; empty when unbound.
    pub fn label(&self, id: &str) -> String {
        self.bindings(id).iter().map(Chord::label).collect::<Vec<_>>().join(", ")
    }

    /// The defaults of `id`.
    pub fn defaults(&self, id: &str) -> Vec<Chord> {
        self.index(id).and_then(|i| self.specs.get(i)).map(|s| s.defaults.iter().filter_map(|t| Chord::parse(t)).collect()).unwrap_or_default()
    }

    /// Replaces the bindings of `id`; chords that don't suit the action are dropped. Returns
    /// false for an unknown id.
    pub fn set(&mut self, id: &str, chords: Vec<Chord>) -> bool {
        let Some(i) = self.index(id) else { return false };
        let (Some(spec), Some(slot)) = (self.specs.get(i), self.bindings.get_mut(i)) else { return false };
        let mut kept: Vec<Chord> = Vec::new();
        for c in chords.into_iter().filter(|c| spec.kind.accepts(c)) {
            if !kept.contains(&c) {
                kept.push(c);
            }
        }
        *slot = kept;
        true
    }

    pub fn reset(&mut self, id: &str) {
        let d = self.defaults(id);
        self.set(id, d);
    }

    pub fn reset_all(&mut self) {
        for s in self.specs {
            self.reset(s.id);
        }
    }

    pub fn is_default(&self, id: &str) -> bool {
        self.bindings(id) == self.defaults(id).as_slice()
    }

    /// What to store: the actions whose bindings differ from their defaults.
    pub fn overrides(&self) -> BTreeMap<String, Vec<String>> {
        self.specs.iter().filter(|s| !self.is_default(s.id)).map(|s| (s.id.to_owned(), self.bindings(s.id).iter().map(Chord::to_string).collect())).collect()
    }

    /// Other actions that share a binding with `id`; key actions only clash with key actions,
    /// gestures with gestures.
    pub fn conflicts(&self, id: &str) -> Vec<&'static ActionSpec> {
        let mine = self.bindings(id);
        self.specs.iter().filter(|s| s.id != id && self.bindings(s.id).iter().any(|c| mine.contains(c))).collect()
    }

    /// Takes this frame's key presses that trigger a [`Kind::Press`] action out of the input and
    /// returns those actions, in order. Modifiers must match exactly.
    pub fn take_presses(&self, input: &mut InputState) -> Vec<&'static str> {
        let mut hits = Vec::new();
        input.events.retain(|e| {
            let Event::Key { key, pressed: true, modifiers, .. } = e else { return true };
            let chord = Chord::key(Mods::from(*modifiers), *key);
            match self.action_for(&chord, Kind::Press) {
                Some(id) => {
                    hits.push(id);
                    false
                }
                None => true,
            }
        });
        hits
    }

    /// Whether a [`Kind::Hold`] action's key is down with exactly its modifiers.
    #[allow(dead_code)] // for the stencil and hide keys, which arrive with the paint tools
    pub fn held(&self, input: &InputState, id: &str) -> bool {
        let mods = Mods::from(input.modifiers);
        self.bindings(id).iter().any(|c| c.mods == mods && matches!(c.input, Input::Key(k) if input.key_down(k)))
    }

    /// The gesture action for a drag with `button` and `modifiers` held, if one is bound.
    pub fn drag(&self, modifiers: Modifiers, button: PointerButton) -> Option<&'static str> {
        self.action_for(&Chord { mods: Mods::from(modifiers), input: Input::Drag(button) }, Kind::Gesture)
    }

    /// The gesture action for the wheel with `modifiers` held, if one is bound.
    pub fn wheel(&self, modifiers: Modifiers) -> Option<&'static str> {
        self.action_for(&Chord { mods: Mods::from(modifiers), input: Input::Wheel }, Kind::Gesture)
    }

    fn action_for(&self, chord: &Chord, kind: Kind) -> Option<&'static str> {
        self.specs.iter().zip(&self.bindings).find(|(s, b)| s.kind == kind && b.contains(chord)).map(|(s, _)| s.id)
    }
}

fn parse_all<T: AsRef<str>>(spec: &ActionSpec, texts: &[T], problems: &mut Vec<String>) -> Vec<Chord> {
    let mut out: Vec<Chord> = Vec::new();
    for t in texts {
        match Chord::parse(t.as_ref()) {
            Some(c) if spec.kind.accepts(&c) => {
                if !out.contains(&c) {
                    out.push(c);
                }
            }
            _ => problems.push(format!("Shortcut '{}' for '{}' was not understood and was ignored.", t.as_ref(), spec.label)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{self, Key, RawInput};

    const SPECS: &[ActionSpec] = &[
        ActionSpec { id: "open", label: "Open", group: "File", kind: Kind::Press, defaults: &["Mod+O"] },
        ActionSpec { id: "tool.fill", label: "Fill", group: "Tools", kind: Kind::Press, defaults: &["4"] },
        ActionSpec { id: "stencil", label: "Stencil", group: "Tools", kind: Kind::Hold, defaults: &["S"] },
        ActionSpec { id: "orbit", label: "Orbit", group: "Navigation", kind: Kind::Gesture, defaults: &["Alt+LeftDrag"] },
        ActionSpec { id: "snap", label: "Snap", group: "Navigation", kind: Kind::Gesture, defaults: &["Alt+Shift+LeftDrag"] },
        ActionSpec { id: "zoom", label: "Zoom", group: "Navigation", kind: Kind::Gesture, defaults: &["Wheel", "Alt+RightDrag"] },
    ];

    fn table() -> ShortcutTable {
        let (t, problems) = ShortcutTable::new(SPECS, &BTreeMap::new());
        assert!(problems.is_empty(), "{problems:?}");
        t
    }

    fn key_event(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    fn run(ctx: &egui::Context, raw: RawInput, f: impl FnMut(&mut egui::Ui)) {
        ctx.run_ui(raw, f).textures_delta.clear();
    }

    fn input_with(events: Vec<Event>) -> InputState {
        let ctx = egui::Context::default();
        let mut state = None;
        let raw = RawInput { events, ..Default::default() };
        run(&ctx, raw, |ui| state = Some(ui.input(|i| i.clone())));
        state.expect("input")
    }

    #[test]
    fn presses_match_exact_modifiers_and_are_consumed() {
        let t = table();
        let cmd = Modifiers::COMMAND;
        let mut input = input_with(vec![
            key_event(Key::O, cmd),
            key_event(Key::Num4, Modifiers::NONE),
            key_event(Key::Num4, Modifiers::SHIFT),
            key_event(Key::A, Modifiers::NONE),
        ]);
        assert_eq!(t.take_presses(&mut input), vec!["open", "tool.fill"]);
        assert_eq!(input.events.len(), 2, "unmatched events stay for the widgets");
    }

    #[test]
    fn gestures_pick_the_binding_with_exactly_the_held_modifiers() {
        let t = table();
        assert_eq!(t.drag(Modifiers::ALT, PointerButton::Primary), Some("orbit"));
        assert_eq!(t.drag(Modifiers::ALT | Modifiers::SHIFT, PointerButton::Primary), Some("snap"));
        assert_eq!(t.drag(Modifiers::NONE, PointerButton::Primary), None, "plain drags are left for painting");
        assert_eq!(t.drag(Modifiers::ALT, PointerButton::Secondary), Some("zoom"));
        assert_eq!(t.wheel(Modifiers::NONE), Some("zoom"));
    }

    #[test]
    fn rebinding_is_stored_as_an_override_and_reset_removes_it() {
        let mut t = table();
        assert!(t.overrides().is_empty());
        assert!(t.set("open", vec![Chord::parse("Mod+Shift+O").expect("chord")]));
        assert_eq!(t.overrides().get("open"), Some(&vec!["Mod+Shift+O".to_string()]));
        // A key chord can't drive a gesture.
        t.set("orbit", vec![Chord::parse("O").expect("chord")]);
        assert!(t.bindings("orbit").is_empty());
        let (again, problems) = ShortcutTable::new(SPECS, &t.overrides());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(again.bindings("open"), t.bindings("open"));
        assert!(again.bindings("orbit").is_empty(), "an unbound action stays unbound");
        t.reset_all();
        assert!(t.overrides().is_empty());
    }

    #[test]
    fn conflicts_are_found_and_bad_settings_reported() {
        let mut t = table();
        t.set("tool.fill", vec![Chord::parse("Mod+O").expect("chord")]);
        assert_eq!(t.conflicts("open").iter().map(|s| s.id).collect::<Vec<_>>(), vec!["tool.fill"]);
        let mut bad = BTreeMap::new();
        bad.insert("open".to_string(), vec!["Hyper+Q".to_string()]);
        bad.insert("gone".to_string(), vec!["A".to_string()]);
        let (_, problems) = ShortcutTable::new(SPECS, &bad);
        assert_eq!(problems.len(), 2, "{problems:?}");
    }

    #[test]
    fn a_hold_action_is_active_while_its_key_is_down() {
        let t = table();
        let ctx = egui::Context::default();
        let mut held = false;
        run(&ctx, RawInput { events: vec![key_event(Key::S, Modifiers::NONE)], ..Default::default() }, |_| {});
        run(&ctx, RawInput::default(), |ui| held = ui.input(|i| t.held(i, "stencil")));
        assert!(held);
    }
}
