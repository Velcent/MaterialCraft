//! Local kernels ([`StampMode::Local`]): per dab, the values under the dab are gathered onto the dab frame's pixel
//! grid ([`LocalGrid`]), a [`LocalKernel`] edits the grid (smudge, blur), and every texel under the dab takes the
//! change at its frame position, scaled by its visibility, the stroke's factors and the dab's opacity ceiling.
//!
//! Gathering maps each grid cell to a texel: directly for 2D-view and UV-aligned dabs, through a pick of the view
//! for 3D-view dabs (the nearest surface under the cell; surfaces laid on the mesh are gathered as seen on screen).
//! Scattering moves only the change, so texels the kernel leaves alone keep their bits. Local strokes paint the
//! direct copy only: a kernel's state (smudge's carried paint) belongs to one place on the model.
//!
//! [`StampMode::Local`]: super::StampMode::Local

use super::dab::DabSample;
use super::write::Blend;
use crate::mesh::SurfaceMap;
use crate::paint::TiledImage;
use crate::projection::ViewProjector;
use crate::v3::{len, sub};
use std::collections::HashMap;

/// The values under one dab on the dab frame's pixel grid: cell `(x, y)` covers frame pixel `x0 + x`, `y0 + y`
/// (the dab's footprint grid, so `footprint.values[i]` is cell `i`'s coverage).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalGrid {
    pub x0: i32,
    pub y0: i32,
    pub width: usize,
    pub height: usize,
    /// Straight RGBA per cell, row by row, in the space the image blends in (linear light for sRGB images). Cells
    /// off the surface hold their nearest surface cell's value.
    pub values: Vec<[f32; 4]>,
    /// Cells that lie on the surface; changes to the others are dropped.
    pub on_surface: Vec<bool>,
    /// The image keeps coverage in alpha (`values[i][3]`).
    pub has_alpha: bool,
}

impl LocalGrid {
    /// Cell `(x, y)`'s value, clamped to the grid.
    pub fn at(&self, x: i64, y: i64) -> [f32; 4] {
        let x = x.clamp(0, self.width.saturating_sub(1) as i64) as usize;
        let y = y.clamp(0, self.height.saturating_sub(1) as i64) as usize;
        self.values.get(y * self.width + x).copied().unwrap_or([0.0; 4])
    }
}

/// An operation on the values under a dab, run once per dab in stroke order. A kernel instance serves one image
/// for one stroke and may carry state from dab to dab.
pub trait LocalKernel: Send {
    /// Edits `grid` (the values under `dab`; `dab.footprint` lies on the same grid) in place.
    fn dab(&mut self, grid: &mut LocalGrid, dab: &DabSample);
}

/// Blur brush mode (PT-26): softens the values under the stroke, towards a box-blurred copy by coverage × strength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlurKernel {
    /// 0 … 1.
    pub strength: f32,
}

impl LocalKernel for BlurKernel {
    fn dab(&mut self, grid: &mut LocalGrid, dab: &DabSample) {
        let k = if self.strength.is_finite() { self.strength.clamp(0.0, 1.0) } else { 0.0 };
        if k <= 0.0 || grid.values.is_empty() {
            return;
        }
        // a blur radius of a tenth of the dab's, two passes of a box: close to a Gaussian
        let r = ((dab.radius * 0.1).round() as i64).clamp(1, 16);
        let mut blurred = grid.clone();
        for _ in 0..2 {
            blurred.values = box_pass(&blurred, r, true);
            blurred.values = box_pass(&blurred, r, false);
        }
        for (i, (v, b)) in grid.values.iter_mut().zip(&blurred.values).enumerate() {
            let c = dab.footprint.values.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) * k;
            if c > 0.0 {
                *v = [v[0] + (b[0] - v[0]) * c, v[1] + (b[1] - v[1]) * c, v[2] + (b[2] - v[2]) * c, v[3] + (b[3] - v[3]) * c];
            }
        }
    }
}

/// One box-blur pass of radius `r` along rows (`horizontal`) or columns, edges clamped.
fn box_pass(g: &LocalGrid, r: i64, horizontal: bool) -> Vec<[f32; 4]> {
    let n = (2 * r + 1) as f32;
    let mut out = vec![[0.0f32; 4]; g.values.len()];
    for y in 0..g.height as i64 {
        for x in 0..g.width as i64 {
            let mut s = [0.0f32; 4];
            for d in -r..=r {
                let v = if horizontal { g.at(x + d, y) } else { g.at(x, y + d) };
                s = [s[0] + v[0], s[1] + v[1], s[2] + v[2], s[3] + v[3]];
            }
            if let Some(o) = out.get_mut(y as usize * g.width + x as usize) {
                *o = s.map(|c| c / n);
            }
        }
    }
    out
}

/// Where a grid cell's centre lies in the texture: through a texel frame (2D view, UV-aligned dabs) or through a
/// pick of the view (other 3D-view dabs).
pub(super) enum CellMap<'a> {
    /// `texel = anchor + (frame − center) / scale`.
    Texel {
        center: [f32; 2],
        anchor: [f32; 2],
        scale: f32,
    },
    View {
        projector: &'a dyn ViewProjector,
    },
}

/// The view picks of one stroke, by frame pixel (the camera stays put during a stroke).
#[derive(Default)]
pub(super) struct PickCache {
    cells: HashMap<(i32, i32), Option<usize>>,
}

impl PickCache {
    pub fn clear(&mut self) {
        self.cells.clear();
    }
}

impl CellMap<'_> {
    /// The texel index (row-major) under frame pixel `(fx, fy)`, if it lies on the set's surface.
    fn texel(&self, surface: &SurfaceMap, picks: &mut PickCache, fx: i32, fy: i32) -> Option<usize> {
        let (w, h) = (surface.width(), surface.height());
        let f = [fx as f32 + 0.5, fy as f32 + 0.5];
        match self {
            CellMap::Texel { center, anchor, scale } => {
                let t = [anchor[0] + (f[0] - center[0]) / scale, anchor[1] + (f[1] - center[1]) / scale];
                let (x, y) = (t[0].floor(), t[1].floor());
                if !(x >= 0.0 && y >= 0.0 && x < w as f32 && y < h as f32) {
                    return None;
                }
                let i = y as usize * w + x as usize;
                surface.point_index(i).map(|_| i)
            }
            CellMap::View { projector } => *picks.cells.entry((fx, fy)).or_insert_with(|| {
                let hit = projector.pick(f)?;
                let (x, y) = ((hit.uv[0] * w as f32).floor(), (hit.uv[1] * h as f32).floor());
                if !(x >= 0.0 && y >= 0.0 && x < w as f32 && y < h as f32) {
                    return None;
                }
                let (x, y) = (x as usize, y as usize);
                let i = y * w + x;
                let (p, _, _) = surface.point_index(i)?;
                // the texel must be the surface that was hit (not another texture set's part or an overlapping island)
                let step = [x + 1, x.wrapping_sub(1)]
                    .iter()
                    .filter(|&&nx| nx < w)
                    .filter_map(|&nx| surface.point_index(y * w + nx))
                    .map(|(q, _, _)| len(sub(q, p)))
                    .fold(0.0f32, f32::max);
                let tol = (step * 3.0).max(1e-6);
                (step <= 0.0 || len(sub(hit.position, p)) <= tol).then_some(i)
            }),
        }
    }
}

/// The values of `image` under `dab`'s footprint grid, decoded by `blend`, with cells off the surface filled from
/// their neighbours.
pub(super) fn gather(
    image: &TiledImage,
    blend: &Blend,
    surface: &SurfaceMap,
    map: &CellMap,
    picks: &mut PickCache,
    dab: &DabSample,
) -> (LocalGrid, Vec<Option<usize>>) {
    let fp = &dab.footprint;
    let (gw, gh) = (fp.width, fp.height);
    let w = surface.width().max(1);
    let mut texels = Vec::with_capacity(gw * gh);
    let mut values = Vec::with_capacity(gw * gh);
    for gy in 0..gh {
        for gx in 0..gw {
            let t = map.texel(surface, picks, fp.x0 + gx as i32, fp.y0 + gy as i32);
            values.push(t.map_or([0.0; 4], |i| blend.decode(image.get((i % w) as u32, (i / w) as u32))));
            texels.push(t);
        }
    }
    let on_surface: Vec<bool> = texels.iter().map(Option::is_some).collect();
    let mut grid = LocalGrid { x0: fp.x0, y0: fp.y0, width: gw, height: gh, values, on_surface, has_alpha: image.format().layout.has_alpha() };
    fill_off_surface(&mut grid);
    (grid, texels)
}

/// Gives cells off the surface their nearest surface cell's value (grown ring by ring), so kernels that read
/// neighbours (smudge, blur) do not pull in zeros at the model's silhouette.
fn fill_off_surface(g: &mut LocalGrid) {
    let mut known = g.on_surface.clone();
    if !known.iter().any(|k| *k) {
        return;
    }
    let (w, h) = (g.width, g.height);
    loop {
        let mut grown = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if known.get(i).copied().unwrap_or(true) {
                    continue;
                }
                let around = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)];
                let from = around.iter().filter(|(nx, ny)| *nx < w && *ny < h).map(|(nx, ny)| ny * w + nx).find(|&j| known.get(j).copied().unwrap_or(false));
                if let Some(v) = from.and_then(|j| g.values.get(j).copied()) {
                    grown.push((i, v));
                }
            }
        }
        if grown.is_empty() {
            return;
        }
        for (i, v) in grown {
            if let (Some(slot), Some(k)) = (g.values.get_mut(i), known.get_mut(i)) {
                *slot = v;
                *k = true;
            }
        }
    }
}

/// The change a kernel made: `after − before` per cell, zero off the surface.
pub(super) fn delta(before: &LocalGrid, after: &LocalGrid) -> LocalGrid {
    let values = before
        .values
        .iter()
        .zip(&after.values)
        .zip(&before.on_surface)
        .map(|((b, a), on)| if *on { [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]] } else { [0.0; 4] })
        .map(|d| if d.iter().all(|v| v.is_finite()) { d } else { [0.0; 4] })
        .collect();
    LocalGrid { values, ..before.clone() }
}

/// A grid's value at frame position `p`, bilinear between cell centres (0 outside the grid).
pub(super) fn sample(g: &LocalGrid, p: [f32; 2]) -> [f32; 4] {
    let fx = p[0] - g.x0 as f32 - 0.5;
    let fy = p[1] - g.y0 as f32 - 0.5;
    if !(fx > -1.0 && fy > -1.0 && fx < g.width as f32 && fy < g.height as f32) {
        return [0.0; 4];
    }
    let (x, y) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x, fy - y);
    let (x, y) = (x as i64, y as i64);
    let at = |x: i64, y: i64| {
        if x < 0 || y < 0 || x >= g.width as i64 || y >= g.height as i64 {
            return [0.0; 4];
        }
        g.values.get(y as usize * g.width + x as usize).copied().unwrap_or([0.0; 4])
    };
    let (a, b, c, d) = (at(x, y), at(x + 1, y), at(x, y + 1), at(x + 1, y + 1));
    lerp4(lerp4(a, b, tx), lerp4(c, d, tx), ty)
}

/// `p + (q − p) · t` per component.
pub(super) fn lerp4(p: [f32; 4], q: [f32; 4], t: f32) -> [f32; 4] {
    [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, p[2] + (q[2] - p[2]) * t, p[3] + (q[3] - p[3]) * t]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stamp::Footprint;

    fn grid(values: Vec<[f32; 4]>, w: usize) -> LocalGrid {
        let h = values.len() / w;
        LocalGrid { x0: 0, y0: 0, width: w, height: h, on_surface: vec![true; values.len()], values, has_alpha: true }
    }

    #[test]
    fn blur_softens_an_edge_under_the_dab_only() {
        let w = 16;
        let values: Vec<[f32; 4]> = (0..w * w).map(|i| if i % w < 8 { [1.0; 4] } else { [0.0, 0.0, 0.0, 1.0] }).collect();
        let mut g = grid(values.clone(), w);
        let mut cov = vec![0.0; w * w];
        cov.iter_mut().enumerate().filter(|(i, _)| i / w < 8).for_each(|(_, c)| *c = 1.0);
        let dab = DabSample { center: [8.0, 4.0], radius: 20.0, opacity: 1.0, index: 0, footprint: Footprint::new(0, 0, w, w, cov).unwrap() };
        BlurKernel { strength: 1.0 }.dab(&mut g, &dab);
        let v = |x: usize, y: usize| g.values[y * w + x][0];
        assert!(v(7, 2) < 1.0 && v(8, 2) > 0.0, "the edge is soft: {} {}", v(7, 2), v(8, 2));
        assert_eq!(v(7, 12), 1.0, "no coverage, no change");
        assert_eq!(v(0, 2), 1.0, "far from the edge the value stays");
    }

    #[test]
    fn deltas_sample_bilinearly_and_drop_off_surface_cells() {
        let mut before = grid(vec![[0.0; 4]; 4], 2);
        before.on_surface[3] = false;
        let after = grid(vec![[1.0; 4]; 4], 2);
        let d = delta(&before, &after);
        assert_eq!(d.values[0], [1.0; 4]);
        assert_eq!(d.values[3], [0.0; 4]);
        assert_eq!(sample(&d, [0.5, 0.5]), [1.0; 4]);
        assert_eq!(sample(&d, [1.0, 0.5])[0], 1.0);
        assert_eq!(sample(&d, [9.0, 9.0]), [0.0; 4]);
    }

    #[test]
    fn off_surface_cells_take_their_neighbours_values() {
        let mut g = grid(vec![[0.0; 4], [0.5; 4], [0.0; 4]], 3);
        g.on_surface = vec![false, true, false];
        fill_off_surface(&mut g);
        assert_eq!(g.values, vec![[0.5; 4]; 3]);
    }
}
