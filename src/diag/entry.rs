//! One diagnostic message (LG-01, LG-03, LG-10).

use std::fmt;
use std::time::SystemTime;

use super::{Area, Level, Target};

/// One message of the session: when, how serious, from where, what, and optionally about what.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub time: SystemTime,
    pub level: Level,
    pub area: Area,
    pub text: String,
    pub target: Option<Target>,
}

impl Entry {
    /// A new entry stamped with the current time.
    pub fn new(level: Level, area: Area, text: impl Into<String>) -> Entry {
        Entry { time: SystemTime::now(), level, area, text: text.into(), target: None }
    }

    /// The same entry, pointing at `target`.
    pub fn with_target(mut self, target: Target) -> Entry {
        self.target = Some(target);
        self
    }

    /// The same entry with an explicit time (bridged records, tests).
    pub fn at(mut self, time: SystemTime) -> Entry {
        self.time = time;
        self
    }
}

/// One line for copying the log (LG-06): `<UTC time> <level> [<area>] <text> (<target>)`.
/// A multi-line text keeps its lines; a trailing newline is dropped.
impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.text.strip_suffix('\n').unwrap_or(&self.text);
        write!(f, "{} {:<7} [{}] {}", super::timestamp(self.time), self.level.name(), self.area, text)?;
        match &self.target {
            Some(t) => write!(f, " ({t})"),
            None => Ok(()),
        }
    }
}
