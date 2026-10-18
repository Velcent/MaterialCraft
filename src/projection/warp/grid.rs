//! The warp grid as data ([`WarpGrid`]: anchors with image coordinates and Bézier handles) and its edits: the
//! regular grid, moving a point, resetting points, automatic handles and a new size while the grid is regular.
//!
//! Grid space: x, y and z run −1…1 across the projection volume (twice the volume's local coordinates, as
//! Painter's `dddGrid` stores them). Rows run along the grid's `t`, columns along `s`; a regular grid has row 0 at
//! the top (y = 1) and column 0 on the left (x = −1), and its image coordinates run 0…1 the same way, so it shows
//! the image as the planar projection would. A handle (tangent) is an offset from its anchor; a zero handle is
//! automatic (a third of the way towards the neighbour, smoothed through the anchor).

use super::super::modes::{WarpGrid, WarpPoint};
use std::borrow::Cow;
use std::fmt;

/// The most rows or columns a grid may have.
pub const MAX_SIDE: u32 = 64;

/// Handle slots of [`WarpPoint::tangents`].
pub const U_PREV: usize = 0;
pub const U_NEXT: usize = 1;
pub const V_PREV: usize = 2;
pub const V_NEXT: usize = 3;

/// Why a grid cannot be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridError {
    /// Fewer than 2 or more than [`MAX_SIDE`] rows or columns.
    Size { rows: u32, columns: u32 },
    /// The point list does not match `rows × columns` (and is not empty).
    PointCount { expected: usize, found: usize },
    /// A position, image coordinate or handle is not a finite number.
    NotFinite,
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GridError::Size { rows, columns } => write!(f, "a warp grid of {rows} × {columns} points (2 … {MAX_SIDE} each)"),
            GridError::PointCount { expected, found } => write!(f, "a warp grid with {found} points instead of {expected}"),
            GridError::NotFinite => write!(f, "a warp grid with a point that is not a number"),
        }
    }
}

impl WarpGrid {
    /// The regular grid of `rows × columns` points over −1…1 (handles automatic).
    pub fn regular_points(rows: u32, columns: u32) -> Vec<WarpPoint> {
        let (r1, c1) = (rows.max(2) - 1, columns.max(2) - 1);
        let mut out = Vec::with_capacity((rows as usize) * (columns as usize));
        for r in 0..rows {
            for c in 0..columns {
                let (u, v) = (c as f32 / c1 as f32, r as f32 / r1 as f32);
                out.push(WarpPoint { pos: [2.0 * u - 1.0, 1.0 - 2.0 * v, 0.0], uv: [u, v], tangents: [[0.0; 3]; 4] });
            }
        }
        out
    }

    /// Checks the size and the points.
    pub fn validate(&self) -> Result<(), GridError> {
        if !(2..=MAX_SIDE).contains(&self.rows) || !(2..=MAX_SIDE).contains(&self.columns) {
            return Err(GridError::Size { rows: self.rows, columns: self.columns });
        }
        let expected = (self.rows as usize) * (self.columns as usize);
        if !self.points.is_empty() && self.points.len() != expected {
            return Err(GridError::PointCount { expected, found: self.points.len() });
        }
        let finite = |p: &WarpPoint| p.pos.iter().chain(&p.uv).chain(p.tangents.iter().flatten()).all(|v| v.is_finite());
        if !self.points.iter().all(finite) {
            return Err(GridError::NotFinite);
        }
        Ok(())
    }

    /// The anchors row by row (the regular grid when no points are stored).
    pub fn anchors(&self) -> Result<Cow<'_, [WarpPoint]>, GridError> {
        self.validate()?;
        Ok(if self.points.is_empty() { Cow::Owned(WarpGrid::regular_points(self.rows, self.columns)) } else { Cow::Borrowed(&self.points) })
    }

    /// True while no point was moved (the size may still change).
    pub fn is_regular(&self) -> bool {
        self.points.is_empty() || self.points == WarpGrid::regular_points(self.rows, self.columns)
    }

    /// The grid with `rows × columns` regular points; `None` once points were moved (the size is fixed then, PJ-20)
    /// or for a size out of range.
    pub fn resized(&self, rows: u32, columns: u32) -> Option<WarpGrid> {
        (self.is_regular() && (2..=MAX_SIDE).contains(&rows) && (2..=MAX_SIDE).contains(&columns)).then(|| WarpGrid {
            rows,
            columns,
            depth: self.depth,
            points: Vec::new(),
        })
    }

    /// The grid with anchor `index` moved by `delta` (grid units; its handles move with it), or `None` for an
    /// invalid grid or index.
    pub fn with_point_moved(&self, index: usize, delta: [f32; 3]) -> Option<WarpGrid> {
        if !delta.iter().all(|v| v.is_finite()) {
            return None;
        }
        let mut points = self.anchors().ok()?.into_owned();
        let p = points.get_mut(index)?;
        for (c, d) in p.pos.iter_mut().zip(delta) {
            *c += d;
        }
        Some(WarpGrid { points, ..self.clone() })
    }

    /// The position of anchor `index` (grid units).
    pub fn point_position(&self, index: usize) -> Option<[f32; 3]> {
        self.anchors().ok()?.get(index).map(|p| p.pos)
    }

    /// The regular grid of the same size and depth.
    pub fn reset_points(&self) -> WarpGrid {
        WarpGrid { points: Vec::new(), ..self.clone() }
    }

    /// The grid with every handle automatic (positions and image coordinates kept).
    pub fn automatic_tangents(&self) -> WarpGrid {
        let points = self.points.iter().map(|p| WarpPoint { tangents: [[0.0; 3]; 4], ..*p }).collect();
        WarpGrid { points, ..self.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_grids_and_their_edits() {
        let g = WarpGrid::default();
        assert!(g.is_regular() && g.validate().is_ok());
        let pts = WarpGrid::regular_points(3, 3);
        assert_eq!(pts.first().map(|p| (p.pos, p.uv)), Some(([-1.0, 1.0, 0.0], [0.0, 0.0])));
        assert_eq!(pts.get(5).map(|p| (p.pos, p.uv)), Some(([1.0, 0.0, 0.0], [1.0, 0.5])));
        let bigger = g.resized(4, 5).unwrap();
        assert_eq!((bigger.rows, bigger.columns), (4, 5));
        let moved = bigger.with_point_moved(6, [0.1, 0.0, -0.2]).unwrap();
        assert!(!moved.is_regular());
        assert!(moved.resized(3, 3).is_none(), "the size is fixed once a point moved");
        assert_eq!(moved.point_position(6).map(|p| p[2]), Some(-0.2));
        assert!(moved.reset_points().is_regular());
        assert!(g.with_point_moved(9, [0.0; 3]).is_none());
        assert!(g.with_point_moved(0, [f32::NAN, 0.0, 0.0]).is_none());
    }

    #[test]
    fn invalid_grids_are_reported() {
        assert!(matches!(WarpGrid { rows: 1, ..WarpGrid::default() }.validate(), Err(GridError::Size { .. })));
        assert!(matches!(WarpGrid { rows: 65, ..WarpGrid::default() }.validate(), Err(GridError::Size { .. })));
        let few = WarpGrid { points: WarpGrid::regular_points(2, 2), ..WarpGrid::default() };
        assert_eq!(few.validate(), Err(GridError::PointCount { expected: 9, found: 4 }));
        let mut nan = WarpGrid { points: WarpGrid::regular_points(3, 3), ..WarpGrid::default() };
        if let Some(p) = nan.points.get_mut(2) {
            p.uv[0] = f32::INFINITY;
        }
        assert_eq!(nan.validate(), Err(GridError::NotFinite));
    }
}
