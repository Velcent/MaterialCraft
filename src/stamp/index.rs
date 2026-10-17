//! [`TexelIndex`]: object-space bounds of a surface map's texels in cells of [`CELL`]² texels, grouped by image
//! tile, so a dab finds the few texels that may lie under it without visiting the whole map.

use crate::mesh::{SurfaceMap, Vec3};
use crate::paint::TILE_SIZE;

/// Cell edge in texels.
pub const CELL: usize = 16;
/// Cells per tile edge.
pub(super) const CELLS_PER_TILE: usize = TILE_SIZE as usize / CELL;

/// An axis-aligned box in object space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub(super) fn point(p: Vec3) -> Aabb {
        Aabb { min: p, max: p }
    }

    pub(super) fn include(&mut self, p: Vec3) {
        for ((lo, hi), v) in self.min.iter_mut().zip(self.max.iter_mut()).zip(p) {
            *lo = lo.min(v);
            *hi = hi.max(v);
        }
    }

    fn union(a: Option<Aabb>, b: Option<Aabb>) -> Option<Aabb> {
        match (a, b) {
            (Some(mut a), Some(b)) => {
                a.include(b.min);
                a.include(b.max);
                Some(a)
            }
            (a, b) => a.or(b),
        }
    }

    /// True if the boxes share a point.
    pub fn overlaps(&self, other: &Aabb) -> bool {
        (0..3).all(|k| match (self.min.get(k), self.max.get(k), other.min.get(k), other.max.get(k)) {
            (Some(a0), Some(a1), Some(b0), Some(b1)) => a0 <= b1 && b0 <= a1,
            _ => false,
        })
    }

    /// The eight corners.
    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            [a[0], a[1], a[2]],
            [b[0], a[1], a[2]],
            [a[0], b[1], a[2]],
            [b[0], b[1], a[2]],
            [a[0], a[1], b[2]],
            [b[0], a[1], b[2]],
            [a[0], b[1], b[2]],
            [b[0], b[1], b[2]],
        ]
    }

    /// Squared distance from `p` to the box (0 inside).
    pub fn distance_squared(&self, p: Vec3) -> f32 {
        self.min.iter().zip(&self.max).zip(p).map(|((lo, hi), v)| (lo - v).max(0.0).max(v - hi)).map(|d| d * d).sum()
    }
}

/// Cells of a surface map with the bounds of the surface points their texels show. Build once per map.
#[derive(Debug, Clone)]
pub struct TexelIndex {
    width: usize,
    height: usize,
    cells_x: usize,
    tiles_x: usize,
    /// Per cell, row-major; `None` where no texel of the cell lies on the set or its gutter.
    cells: Vec<Option<Aabb>>,
    /// Per image tile, the union of its cells.
    tiles: Vec<Option<Aabb>>,
}

impl TexelIndex {
    pub fn build(map: &SurfaceMap) -> TexelIndex {
        let (w, h) = (map.width(), map.height());
        let (cells_x, cells_y) = (w.div_ceil(CELL), h.div_ceil(CELL));
        let mut rows: Vec<Vec<Option<Aabb>>> = vec![Vec::new(); cells_y];
        crate::paint::par::for_each_mut(&mut rows, |cy, row| {
            *row = (0..cells_x).map(|cx| cell_bounds(map, cx, cy)).collect();
        });
        let cells: Vec<Option<Aabb>> = rows.into_iter().flatten().collect();
        let tile = TILE_SIZE as usize;
        let (tiles_x, tiles_y) = (w.div_ceil(tile), h.div_ceil(tile));
        let mut tiles = vec![None; tiles_x * tiles_y];
        for (i, b) in cells.iter().enumerate() {
            let (cx, cy) = (i % cells_x, i / cells_x);
            if let Some(t) = tiles.get_mut((cy / CELLS_PER_TILE) * tiles_x + cx / CELLS_PER_TILE) {
                *t = Aabb::union(*t, *b);
            }
        }
        TexelIndex { width: w, height: h, cells_x, tiles_x, cells, tiles }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// True if the index was built for a map of this size.
    pub fn fits(&self, map: &SurfaceMap) -> bool {
        self.width == map.width() && self.height == map.height()
    }

    /// The cells whose bounds pass `test`, tested tile by tile first, as `(tile index, cell index)` pairs in tile
    /// order. Tiles index the image's tile grid (`ty * tiles_x + tx`), cells the cell grid.
    pub fn cull(&self, test: impl Fn(&Aabb) -> bool) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let cells_y = self.cells.len() / self.cells_x.max(1);
        for (ti, tile) in self.tiles.iter().enumerate() {
            let Some(tb) = tile else { continue };
            if !test(tb) {
                continue;
            }
            let (tx, ty) = (ti % self.tiles_x, ti / self.tiles_x);
            for cy in ty * CELLS_PER_TILE..((ty + 1) * CELLS_PER_TILE).min(cells_y) {
                for cx in tx * CELLS_PER_TILE..((tx + 1) * CELLS_PER_TILE).min(self.cells_x) {
                    let ci = cy * self.cells_x + cx;
                    if self.cells.get(ci).copied().flatten().is_some_and(|b| test(&b)) {
                        out.push((ti, ci));
                    }
                }
            }
        }
        out
    }

    /// Cells per row.
    pub fn cells_x(&self) -> usize {
        self.cells_x
    }

    /// Image tiles per row.
    pub fn tiles_x(&self) -> usize {
        self.tiles_x
    }
}

fn cell_bounds(map: &SurfaceMap, cx: usize, cy: usize) -> Option<Aabb> {
    let (w, h) = (map.width(), map.height());
    let mut b: Option<Aabb> = None;
    for y in cy * CELL..((cy + 1) * CELL).min(h) {
        for x in cx * CELL..((cx + 1) * CELL).min(w) {
            if let Some((p, _, _)) = map.point_index(y * w + x) {
                match &mut b {
                    Some(b) => b.include(p),
                    None => b = Some(Aabb::point(p)),
                }
            }
        }
    }
    b
}
