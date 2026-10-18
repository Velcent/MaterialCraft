//! The warp grid on screen (PJ-20): its lines along the patch surface (drawn under the volume gizmo too) and, when
//! the grid's points are edited, a knob per anchor.

use materialcraft::mesh::Vec3;
use materialcraft::projection::gizmo::{self, ScreenShape, WarpGridShape};
use materialcraft::projection::{SceneFrame, ViewProjector, Warp};

/// The grid's anchors as knobs (handle = the anchor's index) and its lines, in view coordinates.
pub struct WarpGridView {
    pub knobs: Vec<ScreenShape<usize>>,
    pub lines: Vec<Vec<[f32; 2]>>,
}

impl WarpGridView {
    /// The grid of `warp` seen through `view`; `None` for a grid that cannot be evaluated.
    pub fn build(warp: &Warp, frame: &SceneFrame, view: &dyn ViewProjector) -> Option<WarpGridView> {
        let shape = WarpGridShape::of(warp, frame)?;
        let at = |p: Vec3| view.to_screen(p).map(|s| s.pos);
        let knobs = gizmo::to_screen(&shape.knobs(), at);
        Some(WarpGridView { knobs, lines: screen_lines(&shape.lines, at) })
    }
}

/// `lines` on screen, broken where a point has no place there (behind the camera).
pub fn screen_lines(lines: &[Vec<Vec3>], at: impl Fn(Vec3) -> Option<[f32; 2]>) -> Vec<Vec<[f32; 2]>> {
    let mut out = Vec::new();
    for line in lines {
        let mut run: Vec<[f32; 2]> = Vec::new();
        for p in line {
            match at(*p) {
                Some(s) => run.push(s),
                None if run.len() > 1 => out.push(std::mem::take(&mut run)),
                None => run.clear(),
            }
        }
        if run.len() > 1 {
            out.push(run);
        }
    }
    out
}
