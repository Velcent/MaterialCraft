//! [`compare_depth`]: how well a [`ViewProjector`] agrees with a depth buffer drawn through the same camera (the
//! GPU viewport's read-back): per pixel, does a pick hit what was drawn at the same depth, and per surface point,
//! does the occlusion ray agree with the depth test.

use super::ViewProjector;
use crate::mesh::Vec3;

/// A depth buffer in window depth (0 near … 1 far; 1 where nothing was drawn), rows from the top, on the camera's
/// viewport.
pub struct DepthBuffer<'a> {
    pub width: usize,
    pub height: usize,
    pub depth: &'a [f32],
}

impl DepthBuffer<'_> {
    fn at(&self, x: usize, y: usize) -> Option<f32> {
        if x >= self.width {
            return None;
        }
        self.depth.get(y * self.width + x).copied()
    }
}

/// What one checked pixel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelAgreement {
    /// Neither drew anything.
    Empty,
    /// Both see a surface at the same depth.
    Same,
    /// Only the projector picks a surface.
    ProjectorOnly,
    /// Only the depth buffer has a surface.
    BufferOnly,
    /// Both see a surface, at different depths.
    DepthDiffers,
}

/// The result of [`compare_depth`].
#[derive(Debug, Clone, Default)]
pub struct DepthAgreement {
    /// Checked pixels covered by a surface in the buffer, the projector or both.
    pub covered: usize,
    /// Of those, pixels where both agree on surface and depth.
    pub covered_same: usize,
    /// Checked surface points on screen where the buffer is unambiguous: the point's pixel and its eight
    /// neighbours all lie in front of it (occluded) or none does (visible).
    pub points: usize,
    /// Of those, points where the occlusion ray and the depth test agree.
    pub points_same: usize,
    /// Points within a pixel of an occlusion edge or on a surface steep enough that its depth changes by more than
    /// the tolerance within a pixel: not counted.
    pub points_ambiguous: usize,
    pub occluded_by_ray: usize,
    pub occluded_by_buffer: usize,
    /// One entry per checked pixel, rows from the top (`map_width` per row).
    pub map: Vec<PixelAgreement>,
    pub map_width: usize,
    /// The largest distance (screen units) between a checked pixel centre and its pick projected back to screen.
    pub worst_round_trip: f32,
}

impl DepthAgreement {
    /// Percentage of covered pixels that agree.
    pub fn pixel_percent(&self) -> f32 {
        percent(self.covered_same, self.covered)
    }

    /// Percentage of surface points whose occlusion agrees.
    pub fn point_percent(&self) -> f32 {
        percent(self.points_same, self.points)
    }
}

/// Compares `view` with `buffer` on every `stride`-th pixel and at each of `points` (object space) that lands on
/// screen. Depths agree within `tolerance` world units along the viewing direction.
pub fn compare_depth(view: &dyn ViewProjector, buffer: &DepthBuffer, points: impl IntoIterator<Item = Vec3>, stride: usize, tolerance: f32) -> DepthAgreement {
    let camera = *view.camera();
    let stride = stride.max(1);
    let linear = |d: f32| (d < 1.0).then(|| camera.view_depth_of(d)).flatten();
    let mut out = DepthAgreement { map_width: buffer.width.div_ceil(stride), ..DepthAgreement::default() };
    for y in (0..buffer.height).step_by(stride) {
        for x in (0..buffer.width).step_by(stride) {
            let drawn = buffer.at(x, y).and_then(linear);
            let centre = [x as f32 + 0.5, y as f32 + 0.5];
            let back = view.pick(centre).and_then(|h| view.to_screen(h.position));
            if let Some(b) = back {
                out.worst_round_trip = out.worst_round_trip.max((b.pos[0] - centre[0]).hypot(b.pos[1] - centre[1]));
            }
            let picked = back.map(|s| s.view_depth);
            let class = match (picked, drawn) {
                (None, None) => PixelAgreement::Empty,
                (Some(_), None) => PixelAgreement::ProjectorOnly,
                (None, Some(_)) => PixelAgreement::BufferOnly,
                (Some(a), Some(b)) if (a - b).abs() <= tolerance => PixelAgreement::Same,
                (Some(_), Some(_)) => PixelAgreement::DepthDiffers,
            };
            if class != PixelAgreement::Empty {
                out.covered += 1;
                out.covered_same += usize::from(class == PixelAgreement::Same);
            }
            out.map.push(class);
        }
    }
    for p in points {
        let Some(s) = view.to_screen(p) else { continue };
        let (x, y) = (s.pos[0].floor(), s.pos[1].floor());
        if !(x >= 0.0 && y >= 0.0 && s.depth >= 0.0 && s.depth <= 1.0) {
            continue;
        }
        let (x, y) = (x as usize, y as usize);
        // a point the buffer drew nothing over (outside the drawn silhouette) is not on screen for the GPU
        if buffer.at(x, y).and_then(linear).is_none() {
            continue;
        }
        let (mut nearer, mut seen) = (0, 0);
        for (dx, dy) in (0..9).map(|i| (i % 3, i / 3)) {
            let (Some(nx), Some(ny)) = ((x + dx).checked_sub(1), (y + dy).checked_sub(1)) else { continue };
            let Some(d) = buffer.at(nx, ny) else { continue };
            seen += 1;
            nearer += usize::from(linear(d).is_some_and(|drawn| drawn < s.view_depth - tolerance));
        }
        if nearer != 0 && nearer != seen {
            out.points_ambiguous += 1;
            continue;
        }
        let by_buffer = nearer == seen;
        let by_ray = view.occluded(p);
        out.points += 1;
        out.points_same += usize::from(by_buffer == by_ray);
        out.occluded_by_ray += usize::from(by_ray);
        out.occluded_by_buffer += usize::from(by_buffer);
    }
    out
}

fn percent(part: usize, total: usize) -> f32 {
    if total == 0 { 100.0 } else { 100.0 * part as f32 / total as f32 }
}
