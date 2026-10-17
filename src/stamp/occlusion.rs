//! [`DepthMap`]: the view's depth on the CPU, one value per screen unit, drawn once per stroke (the camera does
//! not move during a stroke). A texel is visible when nothing on screen at its position is nearer than it by more
//! than a tolerance; one lookup instead of a ray cast per texel.

use crate::mesh::Mesh;
use crate::projection::ViewProjector;

/// The nearest view depth (distance in front of the camera, world units) per screen pixel; infinite where nothing
/// was drawn.
#[derive(Debug, Clone)]
pub struct DepthMap {
    width: usize,
    height: usize,
    depth: Vec<f32>,
}

/// One projected vertex: screen position and view depth, or `None` behind the camera.
type Projected = Option<([f32; 2], f32)>;

impl DepthMap {
    /// Draws the triangles of the parts `shown` accepts as `projector` sees them, on its camera's viewport.
    pub fn build(mesh: &Mesh, projector: &dyn ViewProjector, shown: &dyn Fn(usize) -> bool) -> DepthMap {
        let vp = projector.camera().viewport;
        let perspective = !projector.camera().is_orthographic();
        let w = if vp[0].is_finite() { vp[0].ceil().clamp(1.0, 16384.0) as usize } else { 1 };
        let h = if vp[1].is_finite() { vp[1].ceil().clamp(1.0, 16384.0) as usize } else { 1 };
        let parts: Vec<(usize, Vec<Projected>)> = mesh
            .parts
            .iter()
            .enumerate()
            .filter(|(i, _)| shown(*i))
            .map(|(i, p)| (i, p.positions.iter().map(|&v| projector.to_screen(v).map(|s| (s.pos, s.view_depth))).collect()))
            .collect();
        // Horizontal bands drawn in parallel; each band draws every triangle clipped to its rows.
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1).clamp(1, 64);
        let rows_per = h.div_ceil(threads).max(1);
        let mut bands: Vec<(usize, Vec<f32>)> = (0..h.div_ceil(rows_per)).map(|b| (b * rows_per, Vec::new())).collect();
        crate::paint::par::for_each_mut(&mut bands, |_, (y0, buf)| {
            let y1 = (*y0 + rows_per).min(h);
            *buf = vec![f32::INFINITY; (y1 - *y0) * w];
            for (pi, verts) in &parts {
                let Some(part) = mesh.parts.get(*pi) else { continue };
                for tri in &part.triangles {
                    let [a, b, c] = tri.map(|i| verts.get(i as usize).copied().flatten());
                    if let (Some(a), Some(b), Some(c)) = (a, b, c) {
                        draw(buf, w, *y0, y1, [a, b, c], perspective);
                    }
                }
            }
        });
        let depth = bands.into_iter().flat_map(|(_, b)| b).collect();
        DepthMap { width: w, height: h, depth }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// The nearest depth at screen position `p` (infinite off screen or where nothing was drawn).
    #[inline]
    pub fn at(&self, p: [f32; 2]) -> f32 {
        if !(p[0] >= 0.0 && p[1] >= 0.0) {
            return f32::INFINITY;
        }
        let (x, y) = (p[0] as usize, p[1] as usize);
        if x >= self.width || y >= self.height {
            return f32::INFINITY;
        }
        self.depth.get(y * self.width + x).copied().unwrap_or(f32::INFINITY)
    }

    /// True if a point at screen position `p` and view depth `depth` is not hidden behind something more than
    /// `tolerance` nearer.
    #[inline]
    pub fn visible(&self, p: [f32; 2], depth: f32, tolerance: f32) -> bool {
        depth <= self.at(p) + tolerance
    }
}

/// Draws one triangle into rows `y0 .. y1` of `buf` (rows relative to `y0`), keeping the nearest depth. Depth is
/// interpolated as 1 / depth for a perspective camera (that is what is linear on screen) and as it is for an
/// orthographic one.
fn draw(buf: &mut [f32], w: usize, y0: usize, y1: usize, v: [([f32; 2], f32); 3], perspective: bool) {
    let [(a, da), (b, db), (c, dc)] = v;
    if !(da > 0.0 && db > 0.0 && dc > 0.0) {
        return;
    }
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if !area.is_finite() || area.abs() < 1e-12 {
        return;
    }
    let min_x = a[0].min(b[0]).min(c[0]).floor().max(0.0);
    let max_x = a[0].max(b[0]).max(c[0]).ceil().min(w as f32);
    let min_y = a[1].min(b[1]).min(c[1]).floor().max(y0 as f32);
    let max_y = a[1].max(b[1]).max(c[1]).ceil().min(y1 as f32);
    if !(min_x < max_x && min_y < max_y) {
        return;
    }
    let inv = if perspective { [1.0 / da, 1.0 / db, 1.0 / dc] } else { [da, db, dc] };
    let edge = |p: [f32; 2], q: [f32; 2], x: f32, y: f32| (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0]);
    for y in min_y as usize..max_y as usize {
        let py = y as f32 + 0.5;
        let row = (y - y0) * w;
        for x in min_x as usize..max_x as usize {
            let px = x as f32 + 0.5;
            let w0 = edge(b, c, px, py) / area;
            let w1 = edge(c, a, px, py) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let i = w0 * inv[0] + w1 * inv[1] + w2 * inv[2];
            let z = if perspective { 1.0 / i } else { i };
            if let Some(slot) = buf.get_mut(row + x)
                && z < *slot
            {
                *slot = z;
            }
        }
    }
}
