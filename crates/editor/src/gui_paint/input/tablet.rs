//! The platform pen reader (PT-56): `photocraft-tablet` (PhotoCraft, pinned; ADR 0007) reads pressure, tilt,
//! rotation and the eraser end beside winit, which drops them, and writes each sample into the view's
//! [`StylusFeed`]. `photocraft-tablet` is an external crate with its own isolated `unsafe` (only its macOS
//! AppKit monitor); MaterialCraft adds none.
//!
//! - Linux X11: XInput2 raw events on a second X connection (pure Rust). Started only when the window itself is
//!   on X11 (Xwayland counts): on native Wayland `$DISPLAY` is Xwayland, which sees none of this window's input.
//! - Linux Wayland: no pen input in a native window (winit doesn't bind `zwp_tablet_v2`). Start the editor with
//!   `WAYLAND_DISPLAY` unset so winit opens the window through Xwayland, where the X11 reader works.
//! - macOS: an AppKit local event monitor, installed on the main thread once the window exists.
//! - Windows: nothing to start; egui's touch force carries the pressure (no tilt or rotation).

use super::stylus::{PenSample, StylusFeed};

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

/// What starting the reader means on `display` (Linux).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plan {
    /// Start the XInput2 reader.
    X11,
    /// No pen input; tell the user how to get it.
    WaylandHint,
    Nothing,
}

/// The plan for `display`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn plan(display: Option<Display>) -> Plan {
    match display {
        Some(Display::X11) => Plan::X11,
        Some(Display::Wayland) => Plan::WaylandHint,
        _ => Plan::Nothing,
    }
}

/// The running pen reader; dropping it stops the macOS monitor (the X11 thread ends with the process).
pub struct TabletInput {
    #[cfg(target_os = "macos")]
    _monitor: Option<photocraft_tablet::macos::Monitor>,
    /// What happened, for the log.
    pub note: String,
}

impl TabletInput {
    /// Nothing started (tests, headless runs).
    pub fn none() -> TabletInput {
        TabletInput {
            #[cfg(target_os = "macos")]
            _monitor: None,
            note: String::new(),
        }
    }

    /// Starts the reader for the window eframe created (call from the app's creation, on the main thread).
    pub fn start(cc: &eframe::CreationContext<'_>, feed: &StylusFeed) -> TabletInput {
        #[cfg(target_os = "linux")]
        {
            let note = match plan(display_of(cc)) {
                Plan::X11 => match photocraft_tablet::x11::spawn(None, sink(feed.clone())) {
                    Ok(_) => "Pen tablet input: reading XInput2 (X11).".to_string(),
                    Err(e) => format!("Pen tablet input is off ({e}); a pen paints like a mouse."),
                },
                Plan::WaylandHint => "Pen tablet input is off on native Wayland (a pen paints like a mouse, or not at all). \
                     Start MaterialCraft with WAYLAND_DISPLAY unset to run it through Xwayland, where pen pressure and tilt work."
                    .to_string(),
                Plan::Nothing => "Pen tablet input: no supported display server; a pen paints like a mouse.".to_string(),
            };
            TabletInput { note }
        }
        #[cfg(target_os = "macos")]
        {
            let _ = cc;
            match photocraft_tablet::macos::Monitor::install(sink(feed.clone())) {
                Ok(m) => TabletInput { _monitor: Some(m), note: "Pen tablet input: AppKit monitor installed.".into() },
                Err(e) => TabletInput { _monitor: None, note: format!("Pen tablet input is off ({e}); a pen paints like a mouse.") },
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = (cc, feed);
            TabletInput { note: "Pen input: pressure from the system's pen events (no tilt or rotation).".into() }
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
        assert_eq!(plan(Some(Display::X11)), Plan::X11);
        assert_eq!(plan(Some(Display::Wayland)), Plan::WaylandHint);
        assert_eq!(plan(Some(Display::Other)), Plan::Nothing);
        assert_eq!(plan(None), Plan::Nothing);
    }
}
