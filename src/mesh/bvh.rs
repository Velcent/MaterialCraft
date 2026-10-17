//! Bounding volume hierarchy over triangles: nearest ray hits for baking, picking and occlusion tests.
//!
//! The tree only stores triangle indices; the triangles stay with their owner and are read through
//! [`Triangles`], so the same BVH works on the baker's triangle soup and on [`super::MeshPart`]s.

use super::Vec3;
use crate::v3::*;

/// Triangle corners by index, the only thing the BVH needs to know about a mesh.
pub trait Triangles {
    fn triangle_count(&self) -> usize;
    /// The three corner positions of triangle `i` (`i < triangle_count()`).
    fn corners(&self, i: usize) -> [Vec3; 3];
}

impl Triangles for [[Vec3; 3]] {
    fn triangle_count(&self) -> usize {
        self.len()
    }

    fn corners(&self, i: usize) -> [Vec3; 3] {
        self[i]
    }
}

#[derive(Clone, Copy)]
struct BNode {
    min: V,
    max: V,
    a: u32, // leaf: start, inner: left
    b: u32, // leaf: count (>0), inner: right (count==0 flagged by leaf=false)
    leaf: bool,
}

pub struct Bvh {
    nodes: Vec<BNode>,
    idx: Vec<u32>,
}

/// The nearest hit of a ray: distance along the ray direction, triangle index and barycentrics
/// (the point is `p0 * (1 - u - v) + p1 * u + p2 * v`).
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

impl Bvh {
    pub fn build<T: Triangles + ?Sized>(tris: &T) -> Bvh {
        let n = tris.triangle_count();
        let cent: Vec<V> = (0..n)
            .map(|i| {
                let p = tris.corners(i);
                mul(add(add(p[0], p[1]), p[2]), 1.0 / 3.0)
            })
            .collect();
        let mut b = Bvh { nodes: Vec::new(), idx: (0..n as u32).collect() };
        b.rec(tris, &cent, 0, n);
        b
    }

    fn rec<T: Triangles + ?Sized>(&mut self, tris: &T, cent: &[V], lo: usize, hi: usize) -> u32 {
        let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
        let (mut cmn, mut cmx) = ([f32::MAX; 3], [f32::MIN; 3]);
        for &i in &self.idx[lo..hi] {
            for p in &tris.corners(i as usize) {
                for k in 0..3 {
                    mn[k] = mn[k].min(p[k]);
                    mx[k] = mx[k].max(p[k]);
                }
            }
            for k in 0..3 {
                cmn[k] = cmn[k].min(cent[i as usize][k]);
                cmx[k] = cmx[k].max(cent[i as usize][k]);
            }
        }
        let me = self.nodes.len() as u32;
        self.nodes.push(BNode { min: mn, max: mx, a: lo as u32, b: (hi - lo) as u32, leaf: true });
        if hi - lo <= 4 {
            return me;
        }
        let ext = sub(cmx, cmn);
        let axis = if ext[0] >= ext[1] && ext[0] >= ext[2] {
            0
        } else if ext[1] >= ext[2] {
            1
        } else {
            2
        };
        self.idx[lo..hi].sort_by(|&a, &b| cent[a as usize][axis].partial_cmp(&cent[b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal));
        let mid = (lo + hi) / 2;
        let l = self.rec(tris, cent, lo, mid);
        let r = self.rec(tris, cent, mid, hi);
        self.nodes[me as usize].leaf = false;
        self.nodes[me as usize].a = l;
        self.nodes[me as usize].b = r;
        me
    }

    /// The nearest hit with `1e-6 < t < tmax` of the ray `o + t * d`. `tris` must be the triangles the BVH was
    /// built from.
    pub fn intersect<T: Triangles + ?Sized>(&self, tris: &T, o: V, d: V, tmax: f32) -> Option<Hit> {
        let inv = [
            1.0 / if d[0].abs() < 1e-12 { 1e-12 } else { d[0] },
            1.0 / if d[1].abs() < 1e-12 { 1e-12 } else { d[1] },
            1.0 / if d[2].abs() < 1e-12 { 1e-12 } else { d[2] },
        ];
        let mut best: Option<Hit> = None;
        let mut tm = tmax;
        let mut stack = [0u32; 64];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let n = self.nodes[stack[sp] as usize];
            let (mut t0, mut t1) = (0.0f32, tm);
            for k in 0..3 {
                let a = (n.min[k] - o[k]) * inv[k];
                let b = (n.max[k] - o[k]) * inv[k];
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
            if t0 > t1 {
                continue;
            }
            if n.leaf {
                for &ti in &self.idx[n.a as usize..(n.a + n.b) as usize] {
                    let p = tris.corners(ti as usize);
                    let e1 = sub(p[1], p[0]);
                    let e2 = sub(p[2], p[0]);
                    let pv = cross(d, e2);
                    let det = dot(e1, pv);
                    if det.abs() < 1e-14 {
                        continue;
                    }
                    let id = 1.0 / det;
                    let tv = sub(o, p[0]);
                    let u = dot(tv, pv) * id;
                    if !(0.0..=1.0).contains(&u) {
                        continue;
                    }
                    let q = cross(tv, e1);
                    let v = dot(d, q) * id;
                    if v < 0.0 || u + v > 1.0 {
                        continue;
                    }
                    let tt = dot(e2, q) * id;
                    if tt > 1e-6 && tt < tm {
                        tm = tt;
                        best = Some(Hit { t: tt, tri: ti, u, v });
                    }
                }
            } else if sp + 2 <= 64 {
                stack[sp] = n.a;
                stack[sp + 1] = n.b;
                sp += 2;
            }
        }
        best
    }
}
