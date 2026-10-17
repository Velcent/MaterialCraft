//! How serious an entry is (LG-02).

use std::fmt;

/// Severity of an entry, ordered from least to most serious.
///
/// `Debug` exists for messages bridged in from third-party crates when the user asks for more
/// detail; the panel's three looks are info, warning and error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}

impl Level {
    /// Every level, least serious first.
    pub const ALL: [Level; 4] = [Level::Debug, Level::Info, Level::Warning, Level::Error];

    /// Stable lowercase name, for log files and settings.
    pub fn name(self) -> &'static str {
        match self {
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }

    /// Inverse of [`Level::name`], ignoring case and surrounding space; `warn` is accepted too.
    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(Level::Debug),
            "info" => Some(Level::Info),
            "warning" | "warn" => Some(Level::Warning),
            "error" => Some(Level::Error),
            _ => None,
        }
    }

    /// This level's bit in a [`super::LevelMask`].
    pub(super) fn bit(self) -> u8 {
        match self {
            Level::Debug => 1,
            Level::Info => 2,
            Level::Warning => 4,
            Level::Error => 8,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
