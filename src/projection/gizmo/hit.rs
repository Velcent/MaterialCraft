//! Gizmo parts on screen and which one the pointer is over: the GUI draws the same [`ScreenShape`]s this module
//! hit-tests, so what is seen is what is grabbed.

use super::handles::{Shape, ShapeKind};

/// A gizmo part in screen units (from the view's top-left corner, y down).
pub type ScreenShape<H> = Shape<H, [f32; 2]>;

/// The parts of `shapes` on screen through `to_screen` (a point → screen; `None` where it has no place there, e.g.
/// behind the camera). A part with such a point is left out.
pub fn to_screen<H: Copy, P: Copy>(shapes: &[Shape<H, P>], to_screen: impl Fn(P) -> Option<[f32; 2]>) -> Vec<ScreenShape<H>> {
    shapes
        .iter()
        .filter_map(|s| {
            let points: Option<Vec<[f32; 2]>> = s.points.iter().map(|p| to_screen(*p)).collect();
            Some(ScreenShape { handle: s.handle, kind: s.kind, points: points? })
        })
        .collect()
}

/// The handle under `at`: a knob within `knob_radius`, then an area containing `at`, then the line nearest to it
/// within `tolerance`. Among knobs, the last one listed wins ties (the centre handles come last).
pub fn pick<H: Copy>(shapes: &[ScreenShape<H>], at: [f32; 2], tolerance: f32, knob_radius: f32) -> Option<H> {
    let mut best: Option<(u8, f32, H)> = None;
    let mut consider = |rank: u8, d: f32, h: H| {
        if best.is_none_or(|(r, bd, _)| rank < r || (rank == r && d <= bd)) {
            best = Some((rank, d, h));
        }
    };
    for s in shapes {
        match s.kind {
            ShapeKind::Knob => {
                if let Some(p) = s.points.first() {
                    let d = dist(*p, at);
                    if d <= knob_radius {
                        consider(0, d, s.handle);
                    }
                }
            }
            ShapeKind::Area => {
                if inside(&s.points, at) {
                    consider(1, 0.0, s.handle);
                }
            }
            ShapeKind::Line => {
                let d = s.points.windows(2).filter_map(|w| Some(segment_distance(*w.first()?, *w.get(1)?, at))).fold(f32::MAX, f32::min);
                if d <= tolerance {
                    consider(2, d, s.handle);
                }
            }
        }
    }
    best.map(|(_, _, h)| h)
}

fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Distance from `p` to the segment `a`–`b`.
pub fn segment_distance(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    dist([a[0] + t * dx, a[1] + t * dy], p)
}

/// Even-odd point-in-polygon test.
fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut odd = false;
    let n = poly.len();
    for i in 0..n {
        let (Some(a), Some(b)) = (poly.get(i), poly.get((i + n - 1) % n)) else { continue };
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            odd = !odd;
        }
    }
    odd
}
