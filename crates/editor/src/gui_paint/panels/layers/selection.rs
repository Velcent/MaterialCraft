//! [`Selection`]: which layers the panel's operations act on (LS-16) and which paintable target the tools
//! work on (ADR 0008, mc2-q-pt-02c: a paint layer's content or any layer's or folder's mask).

/// What on the focused layer is picked: its content or its mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Part {
    #[default]
    Content,
    Mask,
}

/// Selected layer ids of one texture set, in the order they were picked, plus the focused one (the last
/// clicked) and the anchor of Shift ranges.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    ids: Vec<String>,
    anchor: Option<String>,
    pub part: Part,
    /// A content effect of the focused layer picked in the list (its properties are edited, e.g. a fill
    /// effect's projection); any other click clears it.
    effect: Option<usize>,
}

/// Modifiers of a click on a row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickMods {
    /// Ctrl/Cmd: add or remove one layer.
    pub toggle: bool,
    /// Shift: everything between the anchor and the clicked layer.
    pub range: bool,
}

impl Selection {
    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|s| s == id)
    }

    /// The layer last picked (the one property edits and the tools use).
    pub fn focus(&self) -> Option<&str> {
        self.ids.last().map(String::as_str)
    }

    /// Exactly `ids`, the last one focused.
    pub fn set(&mut self, ids: Vec<String>) {
        self.anchor = ids.last().cloned();
        self.ids = ids;
        self.part = Part::Content;
        self.effect = None;
    }

    pub fn clear(&mut self) {
        self.ids.clear();
        self.anchor = None;
        self.part = Part::Content;
        self.effect = None;
    }

    /// Picks content effect `index` of layer `id` (the layer alone is selected and focused).
    pub fn pick_effect(&mut self, id: &str, index: usize) {
        self.set(vec![id.to_string()]);
        self.effect = Some(index);
    }

    /// The content effect picked on the focused layer.
    pub fn effect(&self) -> Option<usize> {
        self.effect
    }

    /// A click on layer `id`; `order` is the rows' layer ids top to bottom (for Shift ranges).
    pub fn click(&mut self, id: &str, mods: ClickMods, part: Part, order: &[&str]) {
        self.effect = None;
        if mods.range
            && let Some(anchor) = self.anchor.clone()
            && let (Some(a), Some(b)) = (order.iter().position(|s| *s == anchor), order.iter().position(|s| *s == id))
        {
            let (lo, hi) = (a.min(b), a.max(b));
            let mut ids: Vec<String> = order.get(lo..=hi).unwrap_or_default().iter().map(|s| s.to_string()).collect();
            // The clicked layer is focused; the anchor stays for the next Shift-click.
            ids.retain(|s| s != id);
            ids.push(id.to_string());
            self.ids = ids;
        } else if mods.toggle {
            if self.contains(id) {
                self.ids.retain(|s| s != id);
            } else {
                self.ids.push(id.to_string());
            }
            self.anchor = Some(id.to_string());
        } else {
            self.ids = vec![id.to_string()];
            self.anchor = Some(id.to_string());
        }
        self.part = part;
    }

    /// Drops ids that are no longer in the stack (after undo or delete); `exists` says whether an id is.
    pub fn retain(&mut self, exists: impl Fn(&str) -> bool) {
        self.ids.retain(|s| exists(s));
        if self.anchor.as_deref().is_some_and(|a| !exists(a)) {
            self.anchor = self.ids.last().cloned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 5] = ["e", "d", "c", "b", "a"];

    #[test]
    fn plain_ctrl_and_shift_clicks() {
        let mut s = Selection::default();
        s.click("d", ClickMods::default(), Part::Content, &ORDER);
        assert_eq!(s.ids(), ["d"]);
        s.click("b", ClickMods { range: true, ..Default::default() }, Part::Content, &ORDER);
        assert_eq!(s.ids(), ["d", "c", "b"]);
        assert_eq!(s.focus(), Some("b"));
        s.click("e", ClickMods { range: true, ..Default::default() }, Part::Content, &ORDER);
        assert_eq!(s.ids(), ["d", "e"], "ranges count from the anchor");
        s.click("a", ClickMods { toggle: true, ..Default::default() }, Part::Content, &ORDER);
        s.click("d", ClickMods { toggle: true, ..Default::default() }, Part::Content, &ORDER);
        assert_eq!(s.ids(), ["e", "a"]);
        s.click("c", ClickMods::default(), Part::Mask, &ORDER);
        assert_eq!((s.ids(), s.part), (&["c".to_string()][..], Part::Mask));
        s.retain(|id| id != "c");
        assert!(s.ids().is_empty() && s.focus().is_none());
    }
}
