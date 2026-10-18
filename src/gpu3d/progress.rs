//! [`Progress`]: which frame of an accumulation to draw next (PP-02, PP-03). Any change to what the scene shows
//! restarts at the plain frame (sample 0), so a moving view costs one plain frame per paint and nothing more; a
//! still view adds one jittered sample per paint until it has enough, then only re-runs the post stage.
//!
//! A plain frame that lacks what only a resting view gets (smooth-navigation shadows whose map has to be redrawn)
//! is held until the view has rested for [`SETTLE`]: a single paint without movement in the middle of an orbit
//! (the next mouse event hadn't arrived yet) would otherwise draw the shadows for one frame and drop them again on
//! the next, and the shadows flicker while the view turns.

use super::display::Display;
use crate::render3d::View;
use std::time::{Duration, Instant};

/// How long a view must stay unchanged before a held plain frame goes on to its samples.
pub(super) const SETTLE: Duration = Duration::from_millis(250);

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
    /// The 2D view's camera (`None` for the 3D view).
    pub uv: Option<crate::camera::Camera>,
}

impl FrameKey {
    pub fn new(view: &View, display: &Display, opts: (bool, f32), content: u64, size: (i32, i32)) -> FrameKey {
        FrameKey { view: view.clone(), display: display.scene_part(), opts: (opts.0, opts.1.to_bits()), content, size, uv: None }
    }

    /// The key of the 2D view drawn with `camera`.
    pub fn in_uv(self, camera: crate::camera::Camera) -> FrameKey {
        FrameKey { uv: Some(camera), ..self }
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
    /// When `key` was first drawn.
    since: Option<Instant>,
}

impl Progress {
    /// The step for a frame showing `key` at time `now` with `frames` samples wanted; `restart` forces the plain
    /// frame (for read-backs that need the unjittered camera); `hold` says the plain frame drawn last waits for a
    /// resting view, so its samples start only once `key` has stayed the same for [`SETTLE`].
    pub fn next(&mut self, key: FrameKey, frames: u32, restart: bool, hold: bool, now: Instant) -> Step {
        if self.key.as_ref() != Some(&key) {
            self.since = Some(now);
        }
        if restart || self.key.as_ref() != Some(&key) {
            self.key = Some(key);
            self.drawn = 0;
        }
        let resting = self.since.is_none_or(|t| now.saturating_duration_since(t) >= SETTLE);
        if self.drawn == 1 && hold && !resting && frames > 1 {
            return Step::Show;
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
        self.since = None;
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
        let t = Instant::now();
        for i in 0..10 {
            assert_eq!(p.next(key(i as f32 * 0.1, 0.0), 8, false, false, t), Step::Draw { index: 0 });
            assert!(!p.averaged());
        }
    }

    #[test]
    fn a_still_view_accumulates_then_only_shows() {
        let mut p = Progress::default();
        let t = Instant::now();
        let steps: Vec<Step> = (0..10).map(|_| p.next(key(0.0, 0.0), 4, false, false, t)).collect();
        assert_eq!(&steps[..4], &[0, 1, 2, 3].map(|index| Step::Draw { index }));
        assert_eq!(&steps[4..], &[Step::Show; 6]);
        assert!(p.averaged() && !p.refining(4));
        // exposure only changes the post stage
        assert_eq!(p.next(key(0.0, 2.0), 4, false, false, t), Step::Show);
        // a read-back restarts at the plain frame
        assert_eq!(p.next(key(0.0, 2.0), 4, true, false, t), Step::Draw { index: 0 });
        assert!(p.refining(4));
        // without accumulation a still view is drawn once
        let mut q = Progress::default();
        assert_eq!(q.next(key(0.0, 0.0), 1, false, false, t), Step::Draw { index: 0 });
        assert_eq!(q.next(key(0.0, 0.0), 1, false, false, t), Step::Show);
        assert!(!q.refining(1) && !q.averaged());
    }

    #[test]
    fn a_held_plain_frame_waits_for_the_view_to_rest() {
        // an orbit whose mouse events skip a paint now and then: the paints in between show the same view
        let mut p = Progress::default();
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        let mut steps = Vec::new();
        for i in 0..20u64 {
            let yaw = (i / 2) as f32 * 0.01;
            steps.push(p.next(key(yaw, 0.0), 32, false, true, ms(i * 16)));
        }
        // only plain frames: the held view never reached its samples (whose shadows would show for one paint)
        assert!(steps.iter().all(|s| matches!(s, Step::Draw { index: 0 } | Step::Show)), "{steps:?}");
        assert!(p.refining(32));
        // once the view has rested, the samples follow
        let last = key(9.0 * 0.01, 0.0);
        assert_eq!(p.next(last.clone(), 32, false, true, ms(19 * 16 + 100)), Step::Show);
        assert_eq!(p.next(last.clone(), 32, false, true, ms(18 * 16 + SETTLE.as_millis() as u64)), Step::Draw { index: 1 });
        assert_eq!(p.next(last, 32, false, true, ms(2000)), Step::Draw { index: 2 });
        // without a hold, the first unchanged paint goes on at once
        let mut q = Progress::default();
        assert_eq!(q.next(key(0.0, 0.0), 32, false, false, t0), Step::Draw { index: 0 });
        assert_eq!(q.next(key(0.0, 0.0), 32, false, false, t0), Step::Draw { index: 1 });
    }
}
