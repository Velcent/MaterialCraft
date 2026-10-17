//! Symmetry for texture-space dabs (2D-view strokes and UV-aligned 3D-view dabs; ADR 0008 mc2-q-pt-04c): a copy
//! is mirrored *through the mesh*. A texel belongs to a copy where its transformed surface point lies on the
//! surface under the dab; [`TexelLookup`] finds the texel under the dab nearest to that point, and the texel is
//! painted with the dab's coverage there.

use super::index::Aabb;
use crate::mesh::{SurfaceMap, Vec3};
use crate::v3::{dot, len, sub};
use std::collections::HashMap;

/// How far (in typical texel spacings) a transformed point may lie from the nearest texel under the dab.
const REACH: f32 = 1.5;

/// The texels of a dab's texel rectangle by surface position.
pub(super) struct TexelLookup {
    cell: f32,
    reach2: f32,
    grid: HashMap<[i32; 3], Vec<u32>>,
    texels: Vec<(Vec3, Vec3, [u32; 2])>,
    bounds: Aabb,
}

impl TexelLookup {
    /// The texels of rectangle `x0 .. x1` × `y0 .. y1` that show a surface point; `None` if none does.
    pub fn build(surface: &SurfaceMap, x0: usize, y0: usize, x1: usize, y1: usize) -> Option<TexelLookup> {
        let w = surface.width();
        let mut texels = Vec::new();
        let mut steps = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let Some((p, n, _)) = surface.point_index(y * w + x) else { continue };
                if x + 1 < x1
                    && let Some((q, _, _)) = surface.point_index(y * w + x + 1)
                {
                    steps.push(len(sub(q, p)));
                }
                texels.push((p, n, [x as u32, y as u32]));
            }
        }
        let first = texels.first()?.0;
        steps.retain(|s| s.is_finite() && *s > 0.0);
        steps.sort_by(f32::total_cmp);
        // the typical spacing: the median step (gutter and seam jumps are rare outliers)
        let spacing = steps.get(steps.len() / 2).copied().unwrap_or_else(|| texel_guess(&texels).max(1e-6));
        let reach = spacing * REACH;
        let mut bounds = Aabb::point(first);
        let mut grid: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        for (i, (p, _, _)) in texels.iter().enumerate() {
            bounds.include(*p);
            grid.entry(key(*p, reach)).or_default().push(i as u32);
        }
        bounds.min = bounds.min.map(|v| v - reach);
        bounds.max = bounds.max.map(|v| v + reach);
        Some(TexelLookup { cell: reach, reach2: reach * reach, grid, texels, bounds })
    }

    /// The box every point that can find a texel lies in.
    pub fn bounds(&self) -> &Aabb {
        &self.bounds
    }

    /// The texel nearest to surface point `p` within reach whose normal faces the same way as `n` (so the other
    /// side of a thin shell is not taken); ties go to the first texel in row order.
    pub fn nearest(&self, p: Vec3, n: Vec3) -> Option<[u32; 2]> {
        let k = key(p, self.cell);
        let mut best: Option<(f32, u32)> = None;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = self.grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else { continue };
                    for &i in list {
                        let Some((q, m, _)) = self.texels.get(i as usize) else { continue };
                        let d = sub(*q, p);
                        let d2 = dot(d, d);
                        if d2 > self.reach2 || dot(*m, n) <= 0.0 {
                            continue;
                        }
                        if best.is_none_or(|(b, j)| d2 < b || (d2 == b && i < j)) {
                            best = Some((d2, i));
                        }
                    }
                }
            }
        }
        best.and_then(|(_, i)| self.texels.get(i as usize)).map(|t| t.2)
    }
}

fn key(p: Vec3, cell: f32) -> [i32; 3] {
    p.map(|v| (v / cell).floor() as i32)
}

/// A spacing for a rectangle one texel wide: its extent over its texel count.
fn texel_guess(texels: &[(Vec3, Vec3, [u32; 2])]) -> f32 {
    let (Some(a), Some(b)) = (texels.first(), texels.last()) else { return 0.0 };
    len(sub(b.0, a.0)) / texels.len().max(2) as f32
}
