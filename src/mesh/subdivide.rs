//! Uniform subdivision of a mesh part for the viewport's displacement (Paint mode VS-09, VS-10): each level splits
//! every triangle into four at its edge midpoints.
//!
//! - **Positions** follow the scheme over the part's welded topology (vertices at the same position are one
//!   vertex, as in [`super::Topology`]): [`Scheme::Linear`] keeps the surface where it is, [`Scheme::Loop`] is
//!   Loop's smoothing (Warren's weights; boundary edges and vertices follow the boundary curve; non-manifold
//!   vertices stay). Both sides of a UV seam or hard edge get exactly the same new positions, so the result is
//!   as watertight as the input.
//! - **Face-varying data** (UVs, normals, tangents) is interpolated linearly inside each original triangle and
//!   never across a seam: a new vertex on an edge is shared only by triangles that shared both of the edge's
//!   vertices, and takes the mean of their values. Textures painted on the original layout therefore map onto
//!   the subdivided surface exactly as they did before (the equivalent of a linear face-varying boundary rule).
//!   Normals and tangents are renormalised; tangent handedness comes from the edge's first vertex.
//! - **Faces**: the four children of triangle `t` are triangles `4t … 4t + 3`, so source polygons stay whole.

use super::{FaceGroups, MeshPart, Vec3};
use crate::v3::{add, len, mul, norm};
use std::collections::HashMap;

/// How new positions are placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    /// Midpoints: the subdivided surface is the original one.
    #[default]
    Linear,
    /// Loop subdivision: the surface rounds off towards its limit surface.
    Loop,
}

impl MeshPart {
    /// The part subdivided `levels` times. An invalid part (see [`MeshPart::validate`]) is returned unchanged.
    pub fn subdivided(&self, levels: u32, scheme: Scheme) -> MeshPart {
        if levels == 0 || self.validate().is_err() {
            return self.clone();
        }
        let mut part = subdivide_once(self, scheme);
        for _ in 1..levels {
            part = subdivide_once(&part, scheme);
        }
        part
    }

    /// The length of the part's longest triangle edge (0 when it has none).
    pub fn longest_edge(&self) -> f32 {
        let at = |i: u32| self.positions.get(i as usize).copied();
        self.triangles
            .iter()
            .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
            .filter_map(|(a, b)| Some(len(crate::v3::sub(at(a)?, at(b)?))))
            .filter(|l| l.is_finite())
            .fold(0.0, f32::max)
    }
}

/// Welds vertices by exact position: the class of every vertex and each class's position.
fn weld(positions: &[Vec3]) -> (Vec<u32>, Vec<Vec3>) {
    let mut ids: HashMap<[u32; 3], u32> = HashMap::with_capacity(positions.len());
    let mut class = Vec::with_capacity(positions.len());
    let mut class_pos = Vec::new();
    for p in positions {
        let key = p.map(f32::to_bits);
        let next = class_pos.len() as u32;
        let id = *ids.entry(key).or_insert_with(|| {
            class_pos.push(*p);
            next
        });
        class.push(id);
    }
    (class, class_pos)
}

fn key(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

/// Every welded edge with the third corners (welded) of the triangles on it.
fn welded_edges(tris: &[[u32; 3]], class: &[u32]) -> HashMap<(u32, u32), Vec<u32>> {
    let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::with_capacity(tris.len() * 2);
    let c = |v: u32| class.get(v as usize).copied().unwrap_or(0);
    for t in tris {
        for k in 0..3 {
            let (a, b, o) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            edges.entry(key(c(a), c(b))).or_default().push(c(o));
        }
    }
    edges
}

/// Loop's new positions of the original (welded) vertices.
fn loop_even(class_pos: &[Vec3], edges: &HashMap<(u32, u32), Vec<u32>>) -> Vec<Vec3> {
    let n = class_pos.len();
    let mut ring: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut boundary: Vec<Vec<u32>> = vec![Vec::new(); n];
    let mut irregular = vec![false; n];
    for (&(a, b), opposite) in edges {
        if a == b {
            continue;
        }
        for (v, w) in [(a, b), (b, a)] {
            let Some(r) = ring.get_mut(v as usize) else { continue };
            r.push(w);
            match opposite.len() {
                2 => {}
                1 => boundary.get_mut(v as usize).into_iter().for_each(|bnd| bnd.push(w)),
                _ => irregular.get_mut(v as usize).into_iter().for_each(|i| *i = true),
            }
        }
    }
    class_pos
        .iter()
        .enumerate()
        .map(|(v, &p)| {
            let at = |w: &u32| class_pos.get(*w as usize).copied().unwrap_or(p);
            let (r, bnd) = (&ring[v], &boundary[v]);
            if irregular[v] || r.is_empty() {
                p
            } else if !bnd.is_empty() {
                match bnd.as_slice() {
                    [b0, b1] => add(mul(p, 0.75), mul(add(at(b0), at(b1)), 0.125)),
                    _ => p,
                }
            } else {
                let k = r.len() as f32;
                let beta = if r.len() == 3 { 3.0 / 16.0 } else { 3.0 / (8.0 * k) };
                let sum = r.iter().fold([0.0; 3], |s, w| add(s, at(w)));
                add(mul(p, 1.0 - k * beta), mul(sum, beta))
            }
        })
        .collect()
}

/// The position of the new vertex on welded edge `(a, b)`.
fn odd_position(scheme: Scheme, class_pos: &[Vec3], (a, b): (u32, u32), opposite: &[u32]) -> Vec3 {
    let at = |v: u32| class_pos.get(v as usize).copied().unwrap_or([0.0; 3]);
    match (scheme, opposite) {
        (Scheme::Loop, [c, d]) => add(mul(add(at(a), at(b)), 0.375), mul(add(at(*c), at(*d)), 0.125)),
        _ => mul(add(at(a), at(b)), 0.5),
    }
}

fn mid2(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn subdivide_once(part: &MeshPart, scheme: Scheme) -> MeshPart {
    let n = part.positions.len();
    let (class, class_pos) = weld(&part.positions);
    let edges = welded_edges(&part.triangles, &class);
    let even = match scheme {
        Scheme::Linear => class_pos.clone(),
        Scheme::Loop => loop_even(&class_pos, &edges),
    };
    let odd: HashMap<(u32, u32), Vec3> = edges.iter().map(|(&e, opp)| (e, odd_position(scheme, &class_pos, e, opp))).collect();
    let c = |v: u32| class.get(v as usize).copied().unwrap_or(0);

    let mut out = MeshPart {
        name: part.name.clone(),
        material: part.material,
        positions: (0..n).map(|v| even.get(c(v as u32) as usize).copied().unwrap_or([0.0; 3])).collect(),
        normals: part.normals.clone(),
        tangents: part.tangents.clone(),
        uv_sets: part.uv_sets.clone(),
        triangles: Vec::with_capacity(part.triangles.len() * 4),
        faces: FaceGroups::default(),
    };
    let has_normals = part.normals.len() == n;
    let has_tangents = part.tangents.len() == n;
    // face-varying edges: one new vertex per pair of original vertices
    let mut fv: HashMap<(u32, u32), u32> = HashMap::with_capacity(part.triangles.len() * 2);
    let mut split = |a: u32, b: u32, out: &mut MeshPart| -> u32 {
        *fv.entry(key(a, b)).or_insert_with(|| {
            let id = out.positions.len() as u32;
            let (ia, ib) = (a as usize, b as usize);
            out.positions.push(odd.get(&key(c(a), c(b))).copied().unwrap_or([0.0; 3]));
            if has_normals && let (Some(na), Some(nb)) = (part.normals.get(ia), part.normals.get(ib)) {
                let s = add(*na, *nb);
                out.normals.push(if len(s) > 1e-12 { norm(s) } else { *na });
            }
            if has_tangents && let (Some(ta), Some(tb)) = (part.tangents.get(ia), part.tangents.get(ib)) {
                let s = add([ta[0], ta[1], ta[2]], [tb[0], tb[1], tb[2]]);
                let t = if len(s) > 1e-12 { norm(s) } else { [ta[0], ta[1], ta[2]] };
                out.tangents.push([t[0], t[1], t[2], ta[3]]);
            }
            for (set, src) in out.uv_sets.iter_mut().zip(&part.uv_sets) {
                if let (Some(ua), Some(ub)) = (src.get(ia), src.get(ib)) {
                    set.push(mid2(*ua, *ub));
                }
            }
            id
        })
    };
    for &[a, b, cc] in &part.triangles {
        let ab = split(a, b, &mut out);
        let bc = split(b, cc, &mut out);
        let ca = split(cc, a, &mut out);
        out.triangles.extend([[a, ab, ca], [ab, b, bc], [ca, bc, cc], [ab, bc, ca]]);
    }
    if part.faces.triangle_count() == part.triangles.len() && !part.faces.is_per_triangle() {
        out.faces = FaceGroups::from_runs(part.faces.runs().into_iter().map(|(count, size)| (count, size * 4)));
    }
    out
}

#[cfg(test)]
#[path = "subdivide_tests.rs"]
mod tests;
