//! The warp grid in the view (PJ-20): its anchors as knobs and its grid lines along the patch surface
//! ([`WarpGridShape`], object space), and [`WarpPointDrag`], one drag of an anchor in the plane facing the camera.

use super::super::modes::{Warp, WarpGrid};
use super::super::warp::{GRID_PER_LOCAL, surface_for};
use super::handles::{Shape, ShapeKind};
use super::maths::{Ray, hit_plane};
use super::placed::PlacedVolume;
use crate::mesh::Vec3;
use crate::projection::SceneFrame;
use crate::v3::{add, sub};

/// Points per patch along a drawn grid line.
const LINE_STEPS: usize = 8;

/// The grid of a warp projection in object space.
#[derive(Debug, Clone, PartialEq)]
pub struct WarpGridShape {
    /// The anchors, row by row.
    pub anchors: Vec<Vec3>,
    /// Every row and column line along the surface.
    pub lines: Vec<Vec<Vec3>>,
}

impl WarpGridShape {
    /// The grid of `warp` placed in `frame`; `None` for a grid that cannot be evaluated.
    pub fn of(warp: &Warp, frame: &SceneFrame) -> Option<WarpGridShape> {
        let placed = PlacedVolume::of(&warp.volume, frame);
        let to_object = |g: [f64; 3]| placed.point(g.map(|v| v as f32 / GRID_PER_LOCAL));
        let anchors = warp.grid.anchors().ok()?.iter().map(|p| placed.point(p.pos.map(|v| v / GRID_PER_LOCAL))).collect();
        let surface = surface_for(&warp.grid).ok()?;
        let mesh = surface.mesh();
        let (rows, columns) = (warp.grid.rows as usize, warp.grid.columns as usize);
        let line = |fixed: f64, along_s: bool, patches: usize| -> Vec<Vec3> {
            let n = patches * LINE_STEPS;
            (0..=n).map(|k| k as f64 / n as f64).map(|x| to_object(if along_s { mesh.eval(x, fixed) } else { mesh.eval(fixed, x) })).collect()
        };
        let mut lines = Vec::with_capacity(rows + columns);
        for r in 0..rows {
            lines.push(line(r as f64 / (rows - 1).max(1) as f64, true, columns - 1));
        }
        for c in 0..columns {
            lines.push(line(c as f64 / (columns - 1).max(1) as f64, false, rows - 1));
        }
        Some(WarpGridShape { anchors, lines })
    }

    /// One knob per anchor; the handle is the anchor's index.
    pub fn knobs(&self) -> Vec<Shape<usize, Vec3>> {
        self.anchors.iter().enumerate().map(|(i, p)| Shape { handle: i, kind: ShapeKind::Knob, points: vec![*p] }).collect()
    }
}

/// One drag of a warp anchor: it follows the pointer in the plane through it facing the view.
#[derive(Debug, Clone, PartialEq)]
pub struct WarpPointDrag {
    pub index: usize,
    start: WarpGrid,
    placed: PlacedVolume,
    /// The anchor's object-space position at the press.
    anchor: Vec3,
    normal: Vec3,
    /// Where the press met the plane, less the anchor.
    grab: Vec3,
}

impl WarpPointDrag {
    /// Starts dragging anchor `index` of `warp` placed in `frame`, pressed along `ray`.
    pub fn begin(warp: &Warp, frame: &SceneFrame, index: usize, ray: Ray) -> Option<WarpPointDrag> {
        let placed = PlacedVolume::of(&warp.volume, frame);
        let pos = warp.grid.point_position(index)?;
        let anchor = placed.point(pos.map(|v| v / GRID_PER_LOCAL));
        let hit = hit_plane(&ray, anchor, ray.dir)?;
        Some(WarpPointDrag { index, start: warp.grid.clone(), placed, anchor, normal: ray.dir, grab: sub(hit, anchor) })
    }

    /// The grid with the anchor under `ray`; `None` when the ray misses the drag plane or the volume is flat.
    pub fn update(&self, ray: Ray) -> Option<WarpGrid> {
        let hit = hit_plane(&ray, add(self.anchor, self.grab), self.normal)?;
        let target = self.placed.local(sub(hit, self.grab))?;
        let start = self.start.point_position(self.index)?;
        let delta = [0, 1, 2].map(|k| target.get(k).copied().unwrap_or(0.0) * GRID_PER_LOCAL - start.get(k).copied().unwrap_or(0.0));
        self.start.with_point_moved(self.index, delta)
    }
}
