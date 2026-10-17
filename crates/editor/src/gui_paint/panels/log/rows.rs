//! The arithmetic of the virtualised list: which rows a scroll position shows, and when the list
//! follows new entries (LG-05, LG-08). No egui here, so it is tested directly.

use std::ops::Range;

/// Rows laid out above and below the visible ones, so a fast scroll never shows a gap.
pub const OVERSCAN: usize = 2;

/// The rows of a list of `count` rows, each `row_height` tall, that intersect the viewport
/// `[top, top + height)` (content coordinates), widened by [`OVERSCAN`] and clamped to the list.
/// Only these rows are laid out, so the cost of a frame doesn't grow with the log.
pub fn row_window(count: usize, row_height: f32, top: f32, height: f32) -> Range<usize> {
    if count == 0 || row_height.is_nan() || row_height <= 0.0 || !height.is_finite() || !top.is_finite() {
        return 0..0;
    }
    let first = (top.max(0.0) / row_height).floor() as usize;
    let last = ((top.max(0.0) + height.max(0.0)) / row_height).ceil() as usize;
    let start = first.saturating_sub(OVERSCAN).min(count);
    let end = last.saturating_add(OVERSCAN).min(count);
    start..end.max(start)
}

/// The scroll offset that shows the last row at the bottom of a `viewport`-tall view.
pub fn bottom_offset(count: usize, row_height: f32, viewport: f32) -> f32 {
    (count as f32 * row_height - viewport).max(0.0)
}

/// Auto-scroll (LG-05): while on, the list keeps the newest entry in view; scrolling up pauses
/// it, scrolling back to the bottom resumes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Follow {
    pub enabled: bool,
    paused: bool,
}

impl Default for Follow {
    fn default() -> Follow {
        Follow { enabled: true, paused: false }
    }
}

impl Follow {
    /// Whether this frame should scroll to the newest entry.
    pub fn sticks(&self) -> bool {
        self.enabled && !self.paused
    }

    /// On, but held because the user scrolled away from the newest entry.
    pub fn is_paused(&self) -> bool {
        self.enabled && self.paused
    }

    /// After drawing: `at_bottom` says whether the newest row is in view, `user_scrolled`
    /// whether the user moved the list this frame (wheel, scroll bar, drag).
    pub fn observe(&mut self, at_bottom: bool, user_scrolled: bool) {
        if at_bottom {
            self.paused = false;
        } else if user_scrolled {
            self.paused = true;
        }
    }

    /// Back to the newest entry (the "jump to newest" button, or turning auto-scroll on).
    pub fn resume(&mut self) {
        self.paused = false;
    }
}

/// Whether a list scrolled to `offset` shows its last row, with half a row of slack.
pub fn is_at_bottom(count: usize, row_height: f32, offset: f32, viewport: f32) -> bool {
    offset + 0.5 * row_height >= bottom_offset(count, row_height, viewport)
}
