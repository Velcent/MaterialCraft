//! The platform pen reader (PT-56): `photocraft-tablet` (PhotoCraft, pinned; ADR 0007) reads pressure, tilt,
//! rotation and the eraser end beside winit, which drops them, and writes each sample into the view's
//! [`StylusFeed`]. `photocraft-tablet` is an external crate with its own isolated `unsafe` (only its macOS
//! AppKit monitor); MaterialCraft adds none.
//!
//! - Linux X11: XInput2 raw events on a second X connection (pure Rust). Started only when the window itself is
//!   on X11 (Xwayland counts): on native Wayland `$DISPLAY` is Xwayland, which sees none of this window's input.
//! - Linux Wayland: no pen input in a native window (winit 0.30 doesn't bind `zwp_tablet_v2`). The window opens
//!   through Xwayland instead when a tablet is connected or the pen settings ask for it ([`super::window_system`]);
//!   otherwise the [`PenStatus`] says why there is no pressure.
//! - macOS: an AppKit local event monitor, installed on the main thread once the window exists.
//! - Windows: nothing to start; egui's touch force carries the pressure (no tilt or rotation).

use super::pen_status::PenStatus;
use super::stylus::{PenSample, StylusFeed};
use super::window_system::WindowChoice;

/// A tablet sample as the stylus's pen sample.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn pen_sample(s: photocraft_tablet::Sample) -> PenSample {
    PenSample { pressure: s.pressure, tilt_x: s.tilt_x, tilt_y: s.tilt_y, rotation: s.rotation, eraser: s.eraser }.sanitized()
}

/// Writes tablet samples into `feed` (`None` = a mouse again).
#[cfg(any(target_os = "linux", target_os = "macos", test))]
fn sink(feed: StylusFeed) -> impl Fn(Option<photocraft_tablet::Sample>) + Send + 'static {
    move |s| feed.set(s.map(pen_sample))
}

/// The display server the window runs on.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    X11,
    Wayland,
    Other,
}

/// The pen status of a window on `display` that opened as `choice` says (Linux); `spawn` starts the X11 reader,
/// `pens` lists the connected pens (both asked only for an X11 window).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_status(display: Option<Display>, choice: WindowChoice, spawn: impl FnOnce() -> Result<(), String>, pens: impl FnOnce() -> Vec<String>) -> PenStatus {
    match display {
        Some(Display::X11) => match spawn() {
            Ok(()) => {
                let via = if choice.xwayland { "XInput2 through Xwayland" } else { "XInput2 (X11)" };
                let pens = pens();
                let how = if pens.is_empty() { format!("{via}; no pen tablet connected yet") } else { format!("{via}; {}", pens.join(", ")) };
                PenStatus::Reading { how }
            }
            Err(e) => PenStatus::Unavailable { why: format!("the XInput2 reader didn't start ({e})"), fix: None },
        },
        Some(Display::Wayland) => PenStatus::native_wayland(choice.reason),
        _ => PenStatus::Unavailable { why: "the window runs on a display server MaterialCraft reads no pen from".to_string(), fix: None },
    }
}

/// The running pen reader; dropping it stops the macOS monitor (the X11 thread ends with the process).
pub struct TabletInput {
    #[cfg(target_os = "macos")]
    _monitor: Option<photocraft_tablet::macos::Monitor>,
    /// Whether pressure reaches the brushes, for the log and the UI.
    pub status: PenStatus,
}

impl TabletInput {
    /// Nothing started (tests, headless runs).
    pub fn none() -> TabletInput {
        TabletInput {
            #[cfg(target_os = "macos")]
            _monitor: None,
            status: PenStatus::NotStarted,
        }
    }

    /// Starts the reader for the window eframe created (call from the app's creation, on the main thread);
    /// `choice` is where the window was asked to open.
    pub fn start(cc: &eframe::CreationContext<'_>, feed: &StylusFeed, choice: WindowChoice) -> TabletInput {
        #[cfg(target_os = "linux")]
        {
            let spawn = || photocraft_tablet::x11::spawn(None, sink(feed.clone())).map(drop).map_err(|e| e.to_string());
            TabletInput { status: linux_status(display_of(cc), choice, spawn, super::tablet_probe::connected_pens) }
        }
        #[cfg(target_os = "macos")]
        {
            let _ = (cc, choice);
            match photocraft_tablet::macos::Monitor::install(sink(feed.clone())) {
                Ok(m) => TabletInput { _monitor: Some(m), status: PenStatus::Reading { how: "AppKit tablet events".into() } },
                Err(e) => TabletInput { _monitor: None, status: PenStatus::Unavailable { why: format!("the AppKit monitor didn't install ({e})"), fix: None } },
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (cc, feed, choice);
            TabletInput { status: PenStatus::Reading { how: "pressure from the system's pen events (no tilt or rotation)".into() } }
        }
    }
}

/// The display server of the window eframe created.
#[cfg(target_os = "linux")]
fn display_of(cc: &eframe::CreationContext<'_>) -> Option<Display> {
    use raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    Some(match cc.display_handle().ok()?.as_raw() {
        RawDisplayHandle::Xlib(_) | RawDisplayHandle::Xcb(_) => Display::X11,
        RawDisplayHandle::Wayland(_) => Display::Wayland,
        _ => Display::Other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_paint::input::window_system::Reason;
    use photocraft_tablet::Sample;

    #[test]
    fn samples_reach_the_feed_sanitized() {
        let feed = StylusFeed::default();
        let f = sink(feed.clone());
        f(Some(Sample { pressure: 0.25, tilt_x: 30.0, tilt_y: -10.0, rotation: -90.0, eraser: true }));
        assert_eq!(feed.get(), Some(PenSample { pressure: 0.25, tilt_x: 30.0, tilt_y: -10.0, rotation: 270.0, eraser: true }));
        f(Some(Sample { pressure: f32::NAN, tilt_x: 500.0, ..Default::default() }));
        assert_eq!(feed.get().map(|s| (s.pressure, s.tilt_x)), Some((1.0, 90.0)));
        f(None);
        assert_eq!(feed.get(), None);
    }

    #[test]
    fn only_an_x11_window_starts_the_x11_reader() {
        let xwayland = WindowChoice { xwayland: true, reason: Reason::TabletFound };
        let never = || -> Result<(), String> { panic!("no X11 reader for this window") };
        let started = linux_status(Some(Display::X11), xwayland, || Ok(()), || vec!["Pen A".into()]);
        assert_eq!(started, PenStatus::Reading { how: "XInput2 through Xwayland; Pen A".into() });
        let x11 = linux_status(Some(Display::X11), WindowChoice::native(), || Ok(()), Vec::new);
        assert_eq!(x11, PenStatus::Reading { how: "XInput2 (X11); no pen tablet connected yet".into() });
        let failed = linux_status(Some(Display::X11), WindowChoice::native(), || Err("no XInput2".into()), Vec::new);
        assert!(!failed.available() && failed.summary().contains("no XInput2"), "{failed:?}");
        let no_tablet = WindowChoice { xwayland: false, reason: Reason::NoTablet };
        assert_eq!(linux_status(Some(Display::Wayland), no_tablet, never, Vec::new), PenStatus::native_wayland(Reason::NoTablet));
        assert!(!linux_status(Some(Display::Other), WindowChoice::native(), never, Vec::new).available());
        assert!(!linux_status(None, WindowChoice::native(), never, Vec::new).available());
    }
}
