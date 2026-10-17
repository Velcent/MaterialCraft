//! Which entries the log panel shows (LG-02, LG-04, LG-10).

use super::{Area, Entry, Level, Tab, Target};

/// A set of levels, for the panel's per-level toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LevelMask(u8);

impl LevelMask {
    pub const NONE: LevelMask = LevelMask(0);
    pub const ALL: LevelMask = LevelMask(0b1111);

    /// `level` and every more serious level.
    pub fn at_least(level: Level) -> LevelMask {
        Level::ALL.iter().filter(|l| **l >= level).fold(LevelMask::NONE, |m, l| m.with(*l, true))
    }

    /// The same set with `level` shown or hidden.
    pub fn with(self, level: Level, shown: bool) -> LevelMask {
        if shown { LevelMask(self.0 | level.bit()) } else { LevelMask(self.0 & !level.bit()) }
    }

    pub fn contains(self, level: Level) -> bool {
        self.0 & level.bit() != 0
    }
}

impl Default for LevelMask {
    fn default() -> LevelMask {
        LevelMask::ALL
    }
}

/// What the panel shows: entries matching every condition that is set. The default shows all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    pub levels: LevelMask,
    pub tab: Option<Tab>,
    pub area: Option<Area>,
    pub target: Option<Target>,
    /// Text to search for; empty matches everything. ASCII letters match case-insensitively,
    /// other characters exactly.
    pub text: String,
}

impl Filter {
    /// Show everything.
    pub fn all() -> Filter {
        Filter::default()
    }

    pub fn levels(mut self, levels: LevelMask) -> Filter {
        self.levels = levels;
        self
    }

    pub fn in_tab(mut self, tab: Tab) -> Filter {
        self.tab = Some(tab);
        self
    }

    pub fn in_area(mut self, area: Area) -> Filter {
        self.area = Some(area);
        self
    }

    pub fn about(mut self, target: Target) -> Filter {
        self.target = Some(target);
        self
    }

    pub fn containing(mut self, text: impl Into<String>) -> Filter {
        self.text = text.into();
        self
    }

    /// Whether `entry` passes every condition.
    pub fn matches(&self, entry: &Entry) -> bool {
        self.levels.contains(entry.level)
            && self.tab.is_none_or(|t| entry.area.tab() == t)
            && self.area.is_none_or(|a| entry.area == a)
            && self.target.as_ref().is_none_or(|t| entry.target.as_ref() == Some(t))
            && contains_ignore_ascii_case(&entry.text, &self.text)
    }
}

/// `needle` occurs in `haystack`, ASCII letters compared case-insensitively. No allocation: the
/// panel runs this on every entry every frame while a search is typed.
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    n.is_empty() || h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ignores_ascii_case_and_handles_odd_input() {
        assert!(contains_ignore_ascii_case("Baking AO failed", "ao FAIL"));
        assert!(contains_ignore_ascii_case("x", ""));
        assert!(!contains_ignore_ascii_case("", "x"));
        assert!(!contains_ignore_ascii_case("ab", "abc"));
        assert!(contains_ignore_ascii_case("Größe 🦀 ok", "🦀 OK"));
    }

    #[test]
    fn level_masks() {
        let m = LevelMask::at_least(Level::Warning);
        assert!(!m.contains(Level::Debug) && !m.contains(Level::Info) && m.contains(Level::Warning) && m.contains(Level::Error));
        assert!(!m.with(Level::Error, false).contains(Level::Error));
        assert_eq!(LevelMask::at_least(Level::Debug), LevelMask::ALL);
    }
}
