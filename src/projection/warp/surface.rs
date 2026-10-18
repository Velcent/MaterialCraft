//! [`WarpSurface`]: a [`WarpGrid`] as a smooth patch surface ([`BezierMesh`]) with the image laid on it, and the
//! inverse map from a point of the grid's x-y plane to the surface parameters (an [`InverseLookup`] seed refined
//! by Newton steps on the exact patch).
//!
//! Control net: each anchor sits at a patch corner; its handles are the control points next to it along the grid
//! lines; the inner control point by a corner is the anchor plus both its handles towards the patch (no twist).
//! Image coordinates are interpolated bilinearly between the four anchors of a cell.

use super::super::modes::{WarpGrid, WarpPoint};
use super::bezier::BezierMesh;
use super::grid::{GridError, U_NEXT, U_PREV, V_NEXT, V_PREV};
use super::lookup::InverseLookup;

/// Newton steps per inverse query.
const NEWTON_STEPS: usize = 10;
/// A query counts as on the surface when the patch point lies this close to it in x-y (grid units).
const ON_SURFACE: f64 = 1e-6;

/// The image on the grid's surface, with its inverse.
#[derive(Debug, Clone, PartialEq)]
pub struct WarpSurface {
    mesh: BezierMesh,
    rows: usize,
    columns: usize,
    /// The anchors' image coordinates, row by row.
    uvs: Vec<[f64; 2]>,
    lookup: InverseLookup,
}

/// Where a query lands on the surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceHit {
    pub s: f64,
    pub t: f64,
    /// The surface point (grid units).
    pub pos: [f64; 3],
    /// Image coordinate there.
    pub uv: [f64; 2],
    /// Image units per grid unit across x-y (the larger direction), for footprints.
    pub image_per_grid: f64,
}

fn f64x3(v: [f32; 3]) -> [f64; 3] {
    v.map(f64::from)
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled_diff(a: [f64; 3], b: [f64; 3], k: f64) -> [f64; 3] {
    [(a[0] - b[0]) * k, (a[1] - b[1]) * k, (a[2] - b[2]) * k]
}

/// The handle of anchor `here` towards `next` (`towards_next`) or `prev`, from the stored offset or automatic.
fn handle(stored: [f32; 3], here: [f64; 3], prev: Option<[f64; 3]>, next: Option<[f64; 3]>, towards_next: bool) -> [f64; 3] {
    if stored != [0.0; 3] {
        return f64x3(stored);
    }
    let forward = match (prev, next) {
        (Some(p), Some(n)) => scaled_diff(n, p, 1.0 / 6.0),
        (None, Some(n)) => scaled_diff(n, here, 1.0 / 3.0),
        (Some(p), None) => scaled_diff(here, p, 1.0 / 3.0),
        (None, None) => [0.0; 3],
    };
    if towards_next { forward } else { forward.map(|v| -v) }
}

impl WarpSurface {
    /// The surface of `grid`, its inverse precomputed (a lookup of `resolution` cells along the larger side).
    pub fn new(grid: &WarpGrid, resolution: usize) -> Result<WarpSurface, GridError> {
        let anchors = grid.anchors()?;
        let (rows, columns) = (grid.rows as usize, grid.columns as usize);
        let mesh = control_net(&anchors, rows, columns);
        let uvs = anchors.iter().map(|p| p.uv.map(f64::from)).collect();
        let lookup = InverseLookup::build(&mesh, resolution);
        Ok(WarpSurface { mesh, rows, columns, uvs, lookup })
    }

    pub fn mesh(&self) -> &BezierMesh {
        &self.mesh
    }

    /// The image coordinate at `(s, t)` and its derivatives along s and t.
    pub fn image_at(&self, s: f64, t: f64) -> ([f64; 2], [f64; 2], [f64; 2]) {
        let cell = |x: f64, n: usize| {
            let cells = n.saturating_sub(1).max(1);
            let f = x.clamp(0.0, 1.0) * cells as f64;
            let i = (f.floor() as usize).min(cells - 1);
            (i, f - i as f64, cells as f64)
        };
        let (c, a, kc) = cell(s, self.columns);
        let (r, b, kr) = cell(t, self.rows);
        let uv = |r: usize, c: usize| self.uvs.get(r * self.columns + c).copied().unwrap_or([0.0; 2]);
        let (p00, p01, p10, p11) = (uv(r, c), uv(r, c + 1), uv(r + 1, c), uv(r + 1, c + 1));
        let component = |q00: f64, q01: f64, q10: f64, q11: f64| {
            (
                (q00 * (1.0 - a) + q01 * a) * (1.0 - b) + (q10 * (1.0 - a) + q11 * a) * b,
                ((q01 - q00) * (1.0 - b) + (q11 - q10) * b) * kc,
                ((q10 - q00) * (1.0 - a) + (q11 - q01) * a) * kr,
            )
        };
        let (u, us, ut) = component(p00[0], p01[0], p10[0], p11[0]);
        let (v, vs, vt) = component(p00[1], p01[1], p10[1], p11[1]);
        ([u, v], [us, vs], [ut, vt])
    }

    /// The surface point whose x and y are `xy` (grid units), or `None` off the surface. Where the surface folds
    /// over itself the lookup prefers the layer nearest +z.
    pub fn locate(&self, xy: [f64; 2]) -> Option<SurfaceHit> {
        let seeds = self.lookup.seeds(xy);
        seeds.into_iter().flatten().find_map(|seed| self.refine(xy, seed))
    }

    fn refine(&self, xy: [f64; 2], seed: [f64; 2]) -> Option<SurfaceHit> {
        let [mut s, mut t] = seed;
        for _ in 0..NEWTON_STEPS {
            let p = self.mesh.sample(s, t);
            let (ex, ey) = (xy[0] - p.pos[0], xy[1] - p.pos[1]);
            if ex.abs() + ey.abs() < 1e-12 {
                break;
            }
            let (a, b, c, d) = (p.ds[0], p.dt[0], p.ds[1], p.dt[1]);
            let det = a * d - b * c;
            if det.abs() < 1e-14 || !det.is_finite() {
                break;
            }
            s = (s + (d * ex - b * ey) / det).clamp(0.0, 1.0);
            t = (t + (a * ey - c * ex) / det).clamp(0.0, 1.0);
        }
        let p = self.mesh.sample(s, t);
        let miss = ((xy[0] - p.pos[0]).powi(2) + (xy[1] - p.pos[1]).powi(2)).sqrt();
        if miss.is_nan() || miss > ON_SURFACE {
            return None;
        }
        let (uv, uv_s, uv_t) = self.image_at(s, t);
        // d(image)/d(xy) = d(image)/d(st) · d(st)/d(xy)
        let (a, b, c, d) = (p.ds[0], p.dt[0], p.ds[1], p.dt[1]);
        let det = a * d - b * c;
        let image_per_grid = if det.abs() > 1e-14 {
            let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
            let along_x = (uv_s[0] * ia + uv_t[0] * ic).hypot(uv_s[1] * ia + uv_t[1] * ic);
            let along_y = (uv_s[0] * ib + uv_t[0] * id).hypot(uv_s[1] * ib + uv_t[1] * id);
            along_x.max(along_y)
        } else {
            1.0
        };
        Some(SurfaceHit { s, t, pos: p.pos, uv, image_per_grid })
    }
}

/// The bicubic control net of `anchors` (`rows × columns`, row by row).
fn control_net(anchors: &[WarpPoint], rows: usize, columns: usize) -> BezierMesh {
    let (nx, ny) = (3 * (columns - 1) + 1, 3 * (rows - 1) + 1);
    let mut points = vec![[0.0f64; 3]; nx * ny];
    let at = |r: isize, c: isize| -> Option<&WarpPoint> {
        (r >= 0 && c >= 0 && (r as usize) < rows && (c as usize) < columns).then(|| anchors.get(r as usize * columns + c as usize)).flatten()
    };
    let pos = |r: isize, c: isize| at(r, c).map(|p| f64x3(p.pos));
    for r in 0..rows as isize {
        for c in 0..columns as isize {
            let (Some(p), Some(here)) = (at(r, c), pos(r, c)) else { continue };
            let (left, right, up, down) = (pos(r, c - 1), pos(r, c + 1), pos(r - 1, c), pos(r + 1, c));
            let hu =
                |di: isize| if di < 0 { handle(p.tangents[U_PREV], here, left, right, false) } else { handle(p.tangents[U_NEXT], here, left, right, true) };
            let hv = |dj: isize| if dj < 0 { handle(p.tangents[V_PREV], here, up, down, false) } else { handle(p.tangents[V_NEXT], here, up, down, true) };
            let (i, j) = (3 * c, 3 * r);
            let mut put = |di: isize, dj: isize, v: [f64; 3]| {
                let (x, y) = (i + di, j + dj);
                if x >= 0
                    && y >= 0
                    && (x as usize) < nx
                    && (y as usize) < ny
                    && let Some(slot) = points.get_mut(y as usize * nx + x as usize)
                {
                    *slot = v;
                }
            };
            put(0, 0, here);
            for d in [-1, 1] {
                put(d, 0, add(here, hu(d)));
                put(0, d, add(here, hv(d)));
            }
            for dj in [-1, 1] {
                for di in [-1, 1] {
                    put(di, dj, add(add(here, hu(di)), hv(dj)));
                }
            }
        }
    }
    let knots = |n: usize| (0..n).map(|k| k as f64 / (n - 1) as f64).collect();
    BezierMesh { us: knots(columns), vs: knots(rows), points }
}
