//! Where the window opens on Linux (step 58): native Wayland or Xwayland.
//!
//! winit 0.30 (eframe 0.36) doesn't bind Wayland's tablet protocol (`zwp_tablet_manager_v2`), so in a native
//! Wayland window the compositor hands the pen over as a plain pointer: no pressure, tilt or rotation. Through
//! Xwayland the window is an X11 window; Xwayland binds the tablet protocol itself and offers the pen as XInput2
//! devices with an "Abs Pressure" valuator, which `photocraft-tablet`'s X11 reader understands ([`super::tablet`]).
//!
//! The choice is made once, before the window exists, from the session (`WAYLAND_DISPLAY`, `DISPLAY`), the user's
//! [`PenWindow`] setting, an optional `MATERIALCRAFT_WINDOW=wayland|xwayland` override for one run, and whether a pen
//! tablet is connected ([`super::tablet_probe`]). Choosing Xwayland only tells winit's event loop builder to use its
//! X11 backend; nothing in the environment is changed.

use crate::gui_settings::PenWindow;

/// The environment variable that overrides the setting for one run.
pub const OVERRIDE_VAR: &str = "MATERIALCRAFT_WINDOW";

/// The display servers the session offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Session {
    /// `WAYLAND_DISPLAY` or `WAYLAND_SOCKET` is set: winit opens a Wayland window by default.
    pub wayland: bool,
    /// `DISPLAY` is set: an X server (Xwayland in a Wayland session) is there.
    pub x11: bool,
}

impl Session {
    /// The session this process runs in.
    pub fn from_env() -> Session {
        let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
        Session { wayland: set("WAYLAND_DISPLAY") || set("WAYLAND_SOCKET"), x11: set("DISPLAY") }
    }
}

/// Why the window opens where it does; says what that means for the pen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Not a Wayland session (X11, macOS, Windows): winit's default is right.
    NotWayland,
    /// Wayland, and a pen tablet was connected at start (setting "Automatic").
    TabletFound,
    /// Wayland, the setting (or the override) asks for Xwayland.
    XwaylandChosen,
    /// Wayland, the setting (or the override) asks for native Wayland.
    WaylandChosen,
    /// Wayland, "Automatic", and no pen tablet was connected at start.
    NoTablet,
    /// Wayland without an X server (`DISPLAY` unset): Xwayland isn't available.
    NoXwayland,
}

/// Where the window opens and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowChoice {
    /// Open the window through Xwayland (winit's X11 backend).
    pub xwayland: bool,
    pub reason: Reason,
}

impl WindowChoice {
    /// The choice for this process: its session, the setting, the override variable and the connected pens.
    pub fn for_this_session(setting: PenWindow) -> WindowChoice {
        let session = Session::from_env();
        let forced = std::env::var(OVERRIDE_VAR).ok().and_then(|v| parse_override(&v));
        // only look for pens when it matters
        let tablet = || !super::tablet_probe::connected_pens().is_empty();
        choose(session, forced.unwrap_or(setting), tablet)
    }

    /// A choice that leaves winit's default alone.
    #[cfg(test)]
    pub fn native() -> WindowChoice {
        WindowChoice { xwayland: false, reason: Reason::NotWayland }
    }

    /// One line for the log.
    pub fn describe(&self) -> &'static str {
        match self.reason {
            Reason::NotWayland => "Window: the session's default display server.",
            Reason::TabletFound => "Window: through Xwayland, because a pen tablet is connected (pen pressure needs it on Wayland).",
            Reason::XwaylandChosen => "Window: through Xwayland, as chosen in the pen settings.",
            Reason::WaylandChosen => "Window: native Wayland, as chosen in the pen settings.",
            Reason::NoTablet => "Window: native Wayland (no pen tablet was connected at start).",
            Reason::NoXwayland => "Window: native Wayland (no Xwayland: DISPLAY is not set).",
        }
    }

    /// eframe's hook that makes winit open the window on its X11 backend, when Xwayland was chosen.
    pub fn event_loop_hook(&self) -> Option<eframe::EventLoopBuilderHook> {
        #[cfg(target_os = "linux")]
        if self.xwayland {
            return Some(Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            }));
        }
        None
    }
}

/// `MATERIALCRAFT_WINDOW`'s value as a setting (unknown values are ignored).
pub fn parse_override(value: &str) -> Option<PenWindow> {
    match value.trim().to_ascii_lowercase().as_str() {
        "wayland" => Some(PenWindow::Wayland),
        "xwayland" | "x11" => Some(PenWindow::Xwayland),
        "auto" => Some(PenWindow::Auto),
        _ => None,
    }
}

/// The choice for `session` with `setting`; `tablet` says whether a pen tablet is connected (asked only for
/// "Automatic" in a Wayland session with Xwayland).
pub fn choose(session: Session, setting: PenWindow, tablet: impl FnOnce() -> bool) -> WindowChoice {
    let pick = |xwayland, reason| WindowChoice { xwayland, reason };
    if !session.wayland {
        return pick(false, Reason::NotWayland);
    }
    if !session.x11 {
        return pick(false, Reason::NoXwayland);
    }
    match setting {
        PenWindow::Wayland => pick(false, Reason::WaylandChosen),
        PenWindow::Xwayland => pick(true, Reason::XwaylandChosen),
        PenWindow::Auto if tablet() => pick(true, Reason::TabletFound),
        PenWindow::Auto => pick(false, Reason::NoTablet),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAYLAND: Session = Session { wayland: true, x11: true };

    #[test]
    fn automatic_opens_xwayland_only_with_a_tablet() {
        assert_eq!(choose(WAYLAND, PenWindow::Auto, || true), WindowChoice { xwayland: true, reason: Reason::TabletFound });
        assert_eq!(choose(WAYLAND, PenWindow::Auto, || false), WindowChoice { xwayland: false, reason: Reason::NoTablet });
    }

    #[test]
    fn the_setting_wins_over_the_tablet_probe() {
        let probe_not_asked = || panic!("the probe is only for Automatic");
        assert_eq!(choose(WAYLAND, PenWindow::Xwayland, probe_not_asked), WindowChoice { xwayland: true, reason: Reason::XwaylandChosen });
        assert_eq!(choose(WAYLAND, PenWindow::Wayland, probe_not_asked), WindowChoice { xwayland: false, reason: Reason::WaylandChosen });
    }

    #[test]
    fn outside_wayland_or_without_xwayland_nothing_changes() {
        let x11 = Session { wayland: false, x11: true };
        for s in [PenWindow::Auto, PenWindow::Wayland, PenWindow::Xwayland] {
            assert_eq!(choose(x11, s, || true).reason, Reason::NotWayland);
            assert!(!choose(x11, s, || true).xwayland);
            assert_eq!(choose(Session::default(), s, || true).reason, Reason::NotWayland);
            let no_x = choose(Session { wayland: true, x11: false }, s, || true);
            assert_eq!((no_x.xwayland, no_x.reason), (false, Reason::NoXwayland));
        }
    }

    #[test]
    fn the_override_variable_names_a_setting() {
        assert_eq!(parse_override("xwayland"), Some(PenWindow::Xwayland));
        assert_eq!(parse_override(" X11 "), Some(PenWindow::Xwayland));
        assert_eq!(parse_override("Wayland"), Some(PenWindow::Wayland));
        assert_eq!(parse_override("auto"), Some(PenWindow::Auto));
        assert_eq!(parse_override("gnome"), None);
        assert_eq!(parse_override(""), None);
    }

    #[test]
    fn only_xwayland_installs_a_hook() {
        assert!(WindowChoice::native().event_loop_hook().is_none());
        let x = WindowChoice { xwayland: true, reason: Reason::TabletFound };
        assert_eq!(x.event_loop_hook().is_some(), cfg!(target_os = "linux"));
    }
}
