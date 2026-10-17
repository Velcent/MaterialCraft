//! Welded vertices and the edge table: which triangles share which edge.

use super::super::Vec3;
use super::super::normals::position_key;
use std::collections::HashMap;

/// One edge between two welded vertices and the triangles that use it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// Welded vertex ids, smaller first.
    pub vertices: [u32; 2],
    /// The first two triangles using the edge (only `[0]` is meaningful when `uses == 1`).
    pub triangles: [u32; 2],
    /// Which edge (`k`) of each of those triangles it is.
    pub sides: [u8; 2],
    /// How many triangle edges lie on it: 1 = open border, 2 = manifold, more = non-manifold.
    pub uses: u32,
}

impl Edge {
    /// The two triangles of a manifold edge.
    pub fn pair(&self) -> Option<(u32, u32)> {
        (self.uses == 2).then_some((self.triangles[0], self.triangles[1]))
    }

    /// The triangle on the other side of a manifold edge from `tri`.
    pub fn other(&self, tri: u32) -> Option<u32> {
        let (a, b) = self.pair()?;
        if a == tri {
            Some(b)
        } else if b == tri {
            Some(a)
        } else {
            None
        }
    }
}

/// Welding and edges of one part.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct EdgeTable {
    welded: Vec<u32>,
    welded_count: usize,
    edges: Vec<Edge>,
    /// Per triangle: its three edge ids, `None` for a triangle with bad indices.
    triangle_edges: Vec<Option<[Option<u32>; 3]>>,
}

impl EdgeTable {
    pub(super) fn build(positions: &[Vec3], triangles: &[[u32; 3]]) -> EdgeTable {
        let mut id_of_key: HashMap<[u32; 3], u32> = HashMap::new();
        let welded: Vec<u32> = positions
            .iter()
            .map(|p| {
                let next = id_of_key.len() as u32;
                *id_of_key.entry(position_key(*p)).or_insert(next)
            })
            .collect();
        let mut table = EdgeTable { welded_count: id_of_key.len(), welded, edges: Vec::new(), triangle_edges: Vec::with_capacity(triangles.len()) };
        let mut edge_of: HashMap<[u32; 2], u32> = HashMap::new();
        for (t, tri) in triangles.iter().enumerate() {
            let w = tri.map(|i| table.welded(i as usize));
            let [Some(w0), Some(w1), Some(w2)] = w else {
                table.triangle_edges.push(None);
                continue;
            };
            let mut ids = [None; 3];
            for (k, (id, (a, b))) in ids.iter_mut().zip([(w0, w1), (w1, w2), (w2, w0)]).enumerate() {
                if a == b {
                    continue;
                }
                let key = [a.min(b), a.max(b)];
                let next = table.edges.len() as u32;
                let e = *edge_of.entry(key).or_insert(next);
                if e == next {
                    table.edges.push(Edge { vertices: key, triangles: [t as u32, 0], sides: [k as u8, 0], uses: 0 });
                }
                if let Some(edge) = table.edges.get_mut(e as usize) {
                    if edge.uses == 1 {
                        edge.triangles[1] = t as u32;
                        edge.sides[1] = k as u8;
                    }
                    edge.uses = edge.uses.saturating_add(1);
                }
                *id = Some(e);
            }
            table.triangle_edges.push(Some(ids));
        }
        table
    }

    pub(super) fn welded(&self, v: usize) -> Option<u32> {
        self.welded.get(v).copied()
    }

    pub(super) fn welded_count(&self) -> usize {
        self.welded_count
    }

    pub(super) fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub(super) fn triangle(&self, tri: usize) -> Option<[Option<u32>; 3]> {
        self.triangle_edges.get(tri).copied().flatten()
    }

    /// Whether both triangles of a manifold edge have the same UVs at each of its two ends (no UV seam). Without
    /// UVs every edge is continuous.
    pub(super) fn uv_continuous(&self, e: &Edge, triangles: &[[u32; 3]], uvs: Option<&[[f32; 2]]>) -> bool {
        let Some(uvs) = uvs else {
            return true;
        };
        let (Some(a), Some(b)) = (self.side_uvs(e, 0, triangles, uvs), self.side_uvs(e, 1, triangles, uvs)) else {
            return false;
        };
        same_uv(a[0], b[0]) && same_uv(a[1], b[1])
    }

    /// The UVs at the edge's two welded vertices (in `e.vertices` order) as seen from side `s`.
    pub(super) fn side_uvs(&self, e: &Edge, s: usize, triangles: &[[u32; 3]], uvs: &[[f32; 2]]) -> Option<[[f32; 2]; 2]> {
        let tri = triangles.get(*e.triangles.get(s)? as usize)?;
        let k = *e.sides.get(s)? as usize;
        let (i, j) = (*tri.get(k)? as usize, *tri.get((k + 1) % 3)? as usize);
        let (ui, uj) = (*uvs.get(i)?, *uvs.get(j)?);
        if self.welded(i)? == e.vertices[0] { Some([ui, uj]) } else { Some([uj, ui]) }
    }
}

/// UVs closer than this are the same point (imports round-trip UVs through text and floats).
const UV_EPS: f32 = 1e-6;

fn same_uv(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() <= UV_EPS && (a[1] - b[1]).abs() <= UV_EPS
}
