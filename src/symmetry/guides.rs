//! Geometry of the symmetry's visual aids (PT-39), in object space: the mirror plane as a rectangle across the
//! mesh's bounds, the radial axis as a segment through them, and where the plane or axis meets the mesh. The
//! viewport projects these to the screen and draws them.

use super::SymmetrySpec;
use crate::mesh::{Bounds, Mesh, Vec3};

/// How far guides reach past the mesh's bounds, as a share of the bounds' largest side.
const MARGIN: f32 = 0.08;

/// What a spec shows in the view.
#[derive(Debug, Clone, PartialEq)]
pub enum Guide {
    /// A mirror plane: the four corners of its rectangle within the bounds, in order around it.
    Plane([Vec3; 4]),
    /// A radial axis: its two ends just outside the bounds.
    Axis([Vec3; 2]),
}

/// The guide of `spec` across `bounds`; `None` without symmetry.
pub fn guide(spec: SymmetrySpec, bounds: Bounds) -> Option<Guide> {
    let size = bounds.size();
    let pad = size.iter().fold(0.0f32, |m, v| m.max(*v)).max(1e-6) * MARGIN;
    let lo = bounds.min.map(|v| v - pad);
    let hi = bounds.max.map(|v| v + pad);
    match spec {
        SymmetrySpec::Mirror { axis, offset } if axis < 3 && offset.is_finite() => {
            let (i, j) = ((axis + 1) % 3, (axis + 2) % 3);
            let corner = |a: f32, b: f32| with(with(with([0.0; 3], axis, offset), i, a), j, b);
            let (li, hi_i, lj, hj) = (pick(lo, i), pick(hi, i), pick(lo, j), pick(hi, j));
            Some(Guide::Plane([corner(li, lj), corner(hi_i, lj), corner(hi_i, hj), corner(li, hj)]))
        }
        SymmetrySpec::Radial { axis, point, .. } if axis < 3 && point.iter().all(|v| v.is_finite()) => {
            Some(Guide::Axis([with(point, axis, pick(lo, axis)), with(point, axis, pick(hi, axis))]))
        }
        _ => None,
    }
}

fn pick(v: [f32; 3], k: usize) -> f32 {
    v.get(k).copied().unwrap_or(0.0)
}

/// `v` with component `k` set to `x`.
fn with(mut v: [f32; 3], k: usize, x: f32) -> [f32; 3] {
    if let Some(c) = v.get_mut(k) {
        *c = x;
    }
    v
}

fn lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Where a spec meets the mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Meeting {
    /// Segments where the mirror plane cuts the mesh's triangles.
    pub lines: Vec<[Vec3; 2]>,
    /// Points where the radial axis passes through the mesh's surface.
    pub points: Vec<Vec3>,
}

/// Where `spec` meets `mesh`: the plane's cut through every triangle, or the axis's crossings of the surface.
pub fn meeting(spec: SymmetrySpec, mesh: &Mesh) -> Meeting {
    let mut out = Meeting::default();
    for part in &mesh.parts {
        let at = |i: u32| part.positions.get(i as usize).copied();
        for t in &part.triangles {
            let (Some(a), Some(b), Some(c)) = (at(t[0]), at(t[1]), at(t[2])) else { continue };
            match spec {
                SymmetrySpec::Mirror { axis, offset } if axis < 3 => {
                    if let Some(seg) = cut([a, b, c], axis, offset) {
                        out.lines.push(seg);
                    }
                }
                SymmetrySpec::Radial { axis, point, .. } if axis < 3 => {
                    if let Some(p) = crossing([a, b, c], axis, point) {
                        out.points.push(p);
                    }
                }
                _ => return out,
            }
        }
    }
    out
}

/// The segment where the plane `p[axis] = offset` cuts triangle `tri`.
fn cut(tri: [Vec3; 3], axis: usize, offset: f32) -> Option<[Vec3; 2]> {
    let d = tri.map(|p| pick(p, axis) - offset);
    let mut hits = Vec::with_capacity(2);
    for (e0, e1) in [(0usize, 1usize), (1, 2), (2, 0)] {
        let (da, db) = (*d.get(e0)?, *d.get(e1)?);
        if (da < 0.0) == (db < 0.0) || da == db {
            continue;
        }
        let t = da / (da - db);
        let (a, b) = (*tri.get(e0)?, *tri.get(e1)?);
        hits.push(lerp(a, b, t));
    }
    match hits.as_slice() {
        [p, q] => Some([*p, *q]),
        _ => None,
    }
}

/// Where the line through `point` along `axis` passes through triangle `tri`.
fn crossing(tri: [Vec3; 3], axis: usize, point: Vec3) -> Option<Vec3> {
    let (i, j) = ((axis + 1) % 3, (axis + 2) % 3);
    let flat = tri.map(|p| [pick(p, i), pick(p, j)]);
    let q = [pick(point, i), pick(point, j)];
    let [a, b, c] = flat;
    let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    if area.abs() < 1e-12 {
        return None;
    }
    let w1 = ((q[0] - a[0]) * (c[1] - a[1]) - (q[1] - a[1]) * (c[0] - a[0])) / area;
    let w2 = ((b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0])) / area;
    let w0 = 1.0 - w1 - w2;
    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
        return None;
    }
    let [ta, tb, tc] = tri;
    let mix = |k: usize| pick(ta, k) * w0 + pick(tb, k) * w1 + pick(tc, k) * w2;
    Some([mix(0), mix(1), mix(2)])
}
