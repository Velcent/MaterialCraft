//! [`InverseLookup`]: the warp surface's parameters `(s, t)` rasterised over the grid's x-y plane once per grid
//! (PJ-21: no per-texel search). The surface is tessellated into small quads; each cell whose centre a quad's
//! triangle covers stores the interpolated `(s, t)`, the layer nearest +z winning where the surface folds. A
//! query reads the cell under the point (and its neighbours as fallbacks) as seeds for Newton steps on the exact
//! patch ([`super::surface::WarpSurface::locate`]).

use super::bezier::BezierMesh;

/// Tessellation steps per patch and direction.
const STEPS_PER_PATCH: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Cell {
    st: [f32; 2],
    z: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InverseLookup {
    /// x-y of the corner of cell (0, 0).
    origin: [f64; 2],
    /// Cell edge length (grid units).
    cell: f64,
    width: usize,
    height: usize,
    cells: Vec<Option<Cell>>,
}

impl InverseLookup {
    /// An empty lookup (every query misses).
    fn empty() -> InverseLookup {
        InverseLookup { origin: [0.0; 2], cell: 1.0, width: 0, height: 0, cells: Vec::new() }
    }

    /// Rasterises `mesh` into about `resolution` cells along the larger side of its x-y extent.
    pub fn build(mesh: &BezierMesh, resolution: usize) -> InverseLookup {
        let resolution = resolution.clamp(8, 2048);
        let (ns, nt) = (mesh.us.len().saturating_sub(1) * STEPS_PER_PATCH, mesh.vs.len().saturating_sub(1) * STEPS_PER_PATCH);
        if ns == 0 || nt == 0 || !mesh.is_valid() {
            return InverseLookup::empty();
        }
        let mut verts = Vec::with_capacity((ns + 1) * (nt + 1));
        for j in 0..=nt {
            for i in 0..=ns {
                let (s, t) = (i as f64 / ns as f64, j as f64 / nt as f64);
                verts.push(([s, t], mesh.eval(s, t)));
            }
        }
        let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
        for (_, p) in &verts {
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        let extent = (hi[0] - lo[0]).max(hi[1] - lo[1]);
        if extent.is_nan() || extent <= 1e-9 || !extent.is_finite() {
            return InverseLookup::empty();
        }
        let cell = extent / resolution as f64;
        let width = (((hi[0] - lo[0]) / cell).ceil() as usize).clamp(1, resolution) + 1;
        let height = (((hi[1] - lo[1]) / cell).ceil() as usize).clamp(1, resolution) + 1;
        let mut lookup = InverseLookup { origin: [lo[0] - 0.5 * cell, lo[1] - 0.5 * cell], cell, width, height, cells: vec![None; width * height] };
        let vert = |i: usize, j: usize| verts.get(j * (ns + 1) + i).copied();
        for j in 0..nt {
            for i in 0..ns {
                let (Some(a), Some(b), Some(c), Some(d)) = (vert(i, j), vert(i + 1, j), vert(i + 1, j + 1), vert(i, j + 1)) else { continue };
                lookup.triangle([a, b, c]);
                lookup.triangle([a, c, d]);
            }
        }
        lookup
    }

    /// Fills the cells whose centres `tri` covers.
    fn triangle(&mut self, tri: [([f64; 2], [f64; 3]); 3]) {
        let [(sa, a), (sb, b), (sc, c)] = tri;
        let det = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if det.abs() < 1e-18 {
            return;
        }
        let to_cell = |v: f64, origin: f64| (v - origin) / self.cell - 0.5;
        let lo = |v: f64, origin: f64| to_cell(v, origin).floor().max(0.0) as usize;
        let hi = |v: f64, origin: f64, n: usize| (to_cell(v, origin).ceil().max(0.0) as usize).min(n.saturating_sub(1));
        let (x0, x1) = (lo(a[0].min(b[0]).min(c[0]), self.origin[0]), hi(a[0].max(b[0]).max(c[0]), self.origin[0], self.width));
        let (y0, y1) = (lo(a[1].min(b[1]).min(c[1]), self.origin[1]), hi(a[1].max(b[1]).max(c[1]), self.origin[1], self.height));
        let eps = -1e-9;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = [self.origin[0] + (x as f64 + 0.5) * self.cell, self.origin[1] + (y as f64 + 0.5) * self.cell];
                let wb = ((p[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (p[1] - a[1])) / det;
                let wc = ((b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1])) / det;
                let wa = 1.0 - wb - wc;
                if wa < eps || wb < eps || wc < eps {
                    continue;
                }
                let st = [(wa * sa[0] + wb * sb[0] + wc * sc[0]) as f32, (wa * sa[1] + wb * sb[1] + wc * sc[1]) as f32];
                let z = (wa * a[2] + wb * b[2] + wc * c[2]) as f32;
                if let Some(slot) = self.cells.get_mut(y * self.width + x)
                    && slot.is_none_or(|old| z > old.z)
                {
                    *slot = Some(Cell { st, z });
                }
            }
        }
    }

    /// Starting parameters for a query at `xy`: the cell under it first, then its neighbours (nearest first).
    pub fn seeds(&self, xy: [f64; 2]) -> [Option<[f64; 2]>; 9] {
        let mut out = [None; 9];
        if self.cells.is_empty() || !xy.iter().all(|v| v.is_finite()) {
            return out;
        }
        let fx = (xy[0] - self.origin[0]) / self.cell;
        let fy = (xy[1] - self.origin[1]) / self.cell;
        if fx < -1.0 || fy < -1.0 || fx > self.width as f64 + 1.0 || fy > self.height as f64 + 1.0 {
            return out;
        }
        let (cx, cy) = (fx.floor() as isize, fy.floor() as isize);
        const ORDER: [(isize, isize); 9] = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1)];
        for (slot, (dx, dy)) in out.iter_mut().zip(ORDER) {
            let (x, y) = (cx + dx, cy + dy);
            if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                *slot = self.cells.get(y as usize * self.width + x as usize).copied().flatten().map(|c| c.st.map(f64::from));
            }
        }
        out
    }
}
