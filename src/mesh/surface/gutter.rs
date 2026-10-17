//! Gutter fill: texels around each island get a surface point too, so filtering and mip levels that read across
//! an island's border see the surface continue instead of empty texels (projections spec, section 3.7).
//!
//! Each gutter texel takes the triangle of an already filled neighbour and extends that triangle's plane to its own
//! UV position, which follows the island's local UV gradient rather than repeating the edge texel.

use super::raster::{NONE, Texel, UvTriangle};
use super::source::SurfaceTriangle;

const UNFILLED: u16 = u16::MAX;

/// Fills up to `width` rings of texels (8-neighbourhood, so a ring is one texel wide in every direction).
pub(super) fn fill(texels: &mut [Texel], tris: &[SurfaceTriangle], w: usize, h: usize, width: u32) {
    let mut ring: Vec<u16> = texels.iter().map(|t| if t.tri == NONE { UNFILLED } else { 0 }).collect();
    let mut frontier: Vec<usize> = (0..texels.len()).filter(|&i| ring.get(i) == Some(&0)).collect();
    let rings = width.min(u32::from(UNFILLED - 1)) as u16;
    for r in 1..=rings {
        let mut next: Vec<usize> = frontier.iter().flat_map(|&i| neighbours(i, w, h)).filter(|&j| ring.get(j) == Some(&UNFILLED)).collect();
        next.sort_unstable();
        next.dedup();
        if next.is_empty() {
            break;
        }
        for &j in &next {
            let Some(texel) = best_source(j, texels, &ring, r, tris, w, h) else {
                continue;
            };
            if let (Some(t), Some(rj)) = (texels.get_mut(j), ring.get_mut(j)) {
                *t = texel;
                *rj = r;
            }
        }
        frontier = next;
    }
}

/// The 8 neighbours of texel `i` that lie inside the image.
fn neighbours(i: usize, w: usize, h: usize) -> impl Iterator<Item = usize> {
    let (x, y) = ((i % w) as i64, (i / w) as i64);
    (-1i64..=1).flat_map(move |dy| (-1i64..=1).map(move |dx| (x + dx, y + dy))).filter_map(move |(nx, ny)| {
        let ok = (nx, ny) != (x, y) && nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64;
        ok.then(|| ny as usize * w + nx as usize)
    })
}

/// Of the neighbours filled before ring `r`, the triangle that needs the least extrapolation to reach texel `j`.
fn best_source(j: usize, texels: &[Texel], ring: &[u16], r: u16, tris: &[SurfaceTriangle], w: usize, h: usize) -> Option<Texel> {
    let (cx, cy) = ((j % w) as f32 + 0.5, (j / w) as f32 + 0.5);
    let mut best: Option<(f32, Texel)> = None;
    for n in neighbours(j, w, h) {
        if ring.get(n).is_none_or(|&rn| rn >= r) {
            continue;
        }
        let Some(src) = texels.get(n) else {
            continue;
        };
        let Some(uvt) = tris.get(src.tri as usize).and_then(|t| UvTriangle::new(t.uv, w, h)) else {
            continue;
        };
        let b = uvt.bary(cx, cy);
        if !b.iter().all(|v| v.is_finite()) {
            continue;
        }
        let outside = -b.iter().copied().fold(f32::MAX, f32::min);
        if best.as_ref().is_none_or(|(o, t)| outside < *o || (outside == *o && src.tri < t.tri)) {
            best = Some((outside, Texel { tri: src.tri, bary: [b[1], b[2]], coverage: 0.0 }));
        }
    }
    best.map(|(_, t)| t)
}
