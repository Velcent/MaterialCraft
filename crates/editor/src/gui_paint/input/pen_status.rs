//! Whether pen pressure reaches the brushes, and if not, why and what the user can do (step 58). Shown in the pen
//! settings and next to the brush's pressure switches, so a pen that paints without pressure is never silent.

use super::window_system::Reason;

/// What the user can do to get pen pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenFix {
    /// Choose "Xwayland" for the window in the pen settings and restart.
    UseXwayland,
    /// Connect the tablet and restart (the window then opens through Xwayland on its own).
    ConnectAndRestart,
}

impl PenFix {
    pub fn text(self) -> &'static str {
        match self {
            PenFix::UseXwayland => "Set \"Window (Wayland)\" in Interface settings → Pen to Xwayland and restart MaterialCraft.",
            PenFix::ConnectAndRestart => {
                "Connect the tablet and restart MaterialCraft, or set \"Window (Wayland)\" in Interface settings → Pen to Xwayland and restart."
            }
        }
    }
}

/// The pen reader's state for the UI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PenStatus {
    /// No reader started (tests, headless runs).
    #[default]
    NotStarted,
    /// A reader runs; `how` names it (and the pens seen at start).
    Reading { how: String },
    /// No pen pressure in this session.
    Unavailable { why: String, fix: Option<PenFix> },
}

impl PenStatus {
    /// True unless pressure is known to be unavailable.
    pub fn available(&self) -> bool {
        !matches!(self, PenStatus::Unavailable { .. })
    }

    /// One line for the log and the pen settings.
    pub fn summary(&self) -> String {
        match self {
            PenStatus::NotStarted => "Pen input: not started.".to_string(),
            PenStatus::Reading { how } => format!("Pen input: {how}."),
            PenStatus::Unavailable { why, fix } => {
                let fix = fix.map(|f| format!(" {}", f.text())).unwrap_or_default();
                format!("No pen pressure: {why}. A pen paints like a mouse (full pressure, no tilt).{fix}")
            }
        }
    }

    /// The status of a native Wayland window, by why the window opened there.
    pub fn native_wayland(reason: Reason) -> PenStatus {
        const WINIT: &str = "the window is a native Wayland window, which gets no pen data (winit 0.30 doesn't support Wayland's tablet protocol)";
        let (why, fix) = match reason {
            Reason::NoTablet => (format!("{WINIT}; no pen tablet was connected when MaterialCraft started"), Some(PenFix::ConnectAndRestart)),
            Reason::WaylandChosen => (format!("{WINIT}; \"Window (Wayland)\" is set to Native Wayland in the pen settings"), Some(PenFix::UseXwayland)),
            Reason::NoXwayland => (format!("{WINIT}; Xwayland isn't available (DISPLAY is not set)"), None),
            Reason::NotWayland | Reason::TabletFound | Reason::XwaylandChosen => (WINIT.to_string(), Some(PenFix::UseXwayland)),
        };
        PenStatus::Unavailable { why, fix }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wayland_reason_says_why_and_offers_a_fix_where_one_exists() {
        for r in [Reason::NoTablet, Reason::WaylandChosen, Reason::NoXwayland, Reason::NotWayland] {
            let s = PenStatus::native_wayland(r);
            assert!(!s.available());
            assert!(s.summary().starts_with("No pen pressure: the window is a native Wayland window"), "{}", s.summary());
        }
        assert!(matches!(PenStatus::native_wayland(Reason::NoTablet), PenStatus::Unavailable { fix: Some(PenFix::ConnectAndRestart), .. }));
        assert!(matches!(PenStatus::native_wayland(Reason::WaylandChosen), PenStatus::Unavailable { fix: Some(PenFix::UseXwayland), .. }));
        assert!(matches!(PenStatus::native_wayland(Reason::NoXwayland), PenStatus::Unavailable { fix: None, .. }));
        assert!(PenStatus::native_wayland(Reason::WaylandChosen).summary().ends_with("restart MaterialCraft."));
    }

    #[test]
    fn a_running_reader_is_available() {
        let s = PenStatus::Reading { how: "XInput2 through Xwayland".into() };
        assert!(s.available() && PenStatus::NotStarted.available());
        assert_eq!(s.summary(), "Pen input: XInput2 through Xwayland.");
    }
}
