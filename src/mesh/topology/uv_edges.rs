//! The UV edges where islands end: island outlines for the 2D view, distance-to-border generators and seam
//! handling.

use super::components::Components;
use super::edges::EdgeTable;

/// Why an island ends at an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UvEdgeKind {
    /// Only one triangle uses the edge: the surface itself ends.
    Border,
    /// The surface continues, but in another place in UV space (another island or another part of this one).
    Seam,
    /// Three or more triangles share the edge.
    NonManifold,
}

/// One triangle edge on an island's outline, in UV space. A seam appears once from each side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UvEdge {
    pub uv: [[f32; 2]; 2],
    pub triangle: u32,
    /// Which edge of the triangle (`k`, from corner `k` to `k + 1`).
    pub side: u8,
    pub island: u32,
    pub kind: UvEdgeKind,
}

/// All outline edges of a part, grouped by island.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UvEdges {
    edges: Vec<UvEdge>,
    /// `edges[start[i]..start[i + 1]]` belong to island `i`.
    start: Vec<usize>,
}

impl UvEdges {
    pub(super) fn build(table: &EdgeTable, triangles: &[[u32; 3]], uvs: Option<&[[f32; 2]]>, islands: &Components) -> UvEdges {
        let mut edges = Vec::new();
        for (t, tri) in triangles.iter().enumerate() {
            let (Some(ids), Some(island)) = (table.triangle(t), islands.of(t)) else {
                continue;
            };
            for (k, id) in ids.iter().enumerate() {
                let Some(e) = id.and_then(|id| table.edges().get(id as usize)) else {
                    continue;
                };
                let kind = match e.uses {
                    1 => UvEdgeKind::Border,
                    2 if table.uv_continuous(e, triangles, uvs) => continue,
                    2 => UvEdgeKind::Seam,
                    _ => UvEdgeKind::NonManifold,
                };
                let uv_at = |i: u32| uvs.and_then(|uv| uv.get(i as usize).copied()).unwrap_or([0.0; 2]);
                let (a, b) = (tri.get(k).copied().unwrap_or(0), tri.get((k + 1) % 3).copied().unwrap_or(0));
                edges.push(UvEdge { uv: [uv_at(a), uv_at(b)], triangle: t as u32, side: k as u8, island, kind });
            }
        }
        edges.sort_by_key(|e| e.island);
        let mut start = Vec::with_capacity(islands.count() as usize + 1);
        for i in 0..=islands.count() {
            start.push(edges.partition_point(|e| e.island < i));
        }
        UvEdges { edges, start }
    }

    /// Every outline edge, island by island.
    pub fn all(&self) -> &[UvEdge] {
        &self.edges
    }

    /// The outline edges of island `island` (empty for an unknown island).
    pub fn of_island(&self, island: u32) -> &[UvEdge] {
        let i = island as usize;
        match (self.start.get(i), self.start.get(i + 1)) {
            (Some(&a), Some(&b)) => self.edges.get(a..b).unwrap_or(&[]),
            _ => &[],
        }
    }
}
