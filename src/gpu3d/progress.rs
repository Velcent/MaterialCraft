//! [`Progress`]: which frame of an accumulation to draw next (PP-02, PP-03). Any change to what the scene shows
//! restarts at the plain frame (sample 0), so a moving view costs one plain frame per paint and nothing more; a
//! still view adds one jittered sample per paint until it has enough, then only re-runs the post stage.

use super::display::Display;
use crate::render3d::View;

/// Everything that changes the drawn scene (not exposure or the tone curve: those only change the post stage).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FrameKey {
    pub view: View,
    pub display: super::display::SceneSettings,
    /// Height self-shadowing and the anisotropic filtering request (as bits).
    pub opts: (bool, u32),
    /// Bumped by every change of the uploaded maps or parts.
    pub content: u64,
    pub size: (i32, i32),
}

impl FrameKey {
    pub fn new(view: &View, display: &Display, opts: (bool, f32), content: u64, size: (i32, i32)) -> FrameKey {
        FrameKey { view: view.clone(), display: display.scene_part(), opts: (opts.0, opts.1.to_bits()), content, size }
    }
}

/// What a paint does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// Draw accumulation sample `index` (0 = the plain frame).
    Draw { index: u32 },
    /// Nothing changed and the accumulation is complete: show it again.
    Show,
}

#[derive(Default)]
pub(super) struct Progress {
    key: Option<FrameKey>,
    /// Samples drawn for `key`.
    drawn: u32,
}

impl Progress {
    /// The step for a frame showing `key` with `frames` samples wanted; `restart` forces the plain frame (for
    /// read-backs that need the unjittered camera).
    pub fn next(&mut self, key: FrameKey, frames: u32, restart: bool) -> Step {
        if restart || self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.drawn = 0;
        }
        if self.drawn < frames.max(1) {
            self.drawn += 1;
            Step::Draw { index: self.drawn - 1 }
        } else {
            Step::Show
        }
    }

    /// True while more samples are wanted for the current frame.
    pub fn refining(&self, frames: u32) -> bool {
        self.drawn < frames.max(1)
    }

    /// True when the picture is the average of several samples (else it is the plain frame itself).
    pub fn averaged(&self) -> bool {
        self.drawn >= 2
    }

    /// Forgets the drawn frame (its target was recreated).
    pub fn invalidate(&mut self) {
        self.key = None;
        self.drawn = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(yaw: f32, stops: f32) -> FrameKey {
        let display = Display { exposure_stops: stops, ..Display::default() };
        FrameKey::new(&View { yaw, ..View::default() }, &display, (true, 0.0), 1, (100, 80))
    }

    #[test]
    fn a_moving_view_draws_only_plain_frames() {
        let mut p = Progress::default();
        for i in 0..10 {
            assert_eq!(p.next(key(i as f32 * 0.1, 0.0), 8, false), Step::Draw { index: 0 });
            assert!(!p.averaged());
        }
    }

    #[test]
    fn a_still_view_accumulates_then_only_shows() {
        let mut p = Progress::default();
        let steps: Vec<Step> = (0..10).map(|_| p.next(key(0.0, 0.0), 4, false)).collect();
        assert_eq!(&steps[..4], &[0, 1, 2, 3].map(|index| Step::Draw { index }));
        assert_eq!(&steps[4..], &[Step::Show; 6]);
        assert!(p.averaged() && !p.refining(4));
        // exposure only changes the post stage
        assert_eq!(p.next(key(0.0, 2.0), 4, false), Step::Show);
        // a read-back restarts at the plain frame
        assert_eq!(p.next(key(0.0, 2.0), 4, true), Step::Draw { index: 0 });
        assert!(p.refining(4));
        // without accumulation a still view is drawn once
        let mut q = Progress::default();
        assert_eq!(q.next(key(0.0, 0.0), 1, false), Step::Draw { index: 0 });
        assert_eq!(q.next(key(0.0, 0.0), 1, false), Step::Show);
        assert!(!q.refining(1) && !q.averaged());
    }
}
