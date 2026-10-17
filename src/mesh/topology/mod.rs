//! Connectivity of one mesh part: welded edges and triangle neighbours, UV islands, connected pieces and the UV
//! edges that outline each island. Face groups (source polygons) are kept separately in [`FaceGroups`].
//!
//! Vertices are welded by exact position (as for normals), so UV seams and hard edges, where importers split
//! vertices, still connect in 3D. Edge `k` of a triangle runs from corner `k` to corner `(k + 1) % 3`.

mod components;
mod edges;
mod faces;
#[cfg(test)]
mod tests;
mod uv_edges;

pub use components::Components;
pub use edges::Edge;
pub use faces::FaceGroups;
pub use uv_edges::{UvEdge, UvEdgeKind, UvEdges};

use super::MeshPart;
use components::Joiner;
use edges::EdgeTable;

/// The topology of one [`MeshPart`] for one UV set. Build it once per mesh revision; queries are cheap.
#[derive(Debug, Clone, PartialEq)]
pub struct Topology {
    edges: EdgeTable,
    islands: Components,
    pieces: Components,
    uv_edges: UvEdges,
}

impl Topology {
    /// The topology of `part` with islands taken from UV set `uv_set`. Triangles with bad indices are left out of
    /// everything. Without that UV set (or with a wrong length), every connected piece counts as one island.
    pub fn of_part(part: &MeshPart, uv_set: usize) -> Topology {
        let uvs = part.uv_sets.get(uv_set).filter(|uv| uv.len() == part.positions.len()).map(Vec::as_slice);
        let edges = EdgeTable::build(&part.positions, &part.triangles);
        let valid: Vec<bool> = (0..part.triangles.len()).map(|t| edges.triangle(t).is_some()).collect();
        let (mut islands, mut pieces) = (Joiner::new(valid.clone()), Joiner::new(valid));
        for e in edges.edges() {
            let Some((a, b)) = e.pair() else {
                continue;
            };
            pieces.join(a, b);
            if edges.uv_continuous(e, &part.triangles, uvs) {
                islands.join(a, b);
            }
        }
        let islands = islands.finish();
        let uv_edges = UvEdges::build(&edges, &part.triangles, uvs, &islands);
        Topology { edges, islands, pieces: pieces.finish(), uv_edges }
    }

    /// The welded vertex of vertex `v`: equal for all vertices at the same position.
    pub fn welded_vertex(&self, v: usize) -> Option<u32> {
        self.edges.welded(v)
    }

    /// How many distinct positions the part has.
    pub fn welded_vertex_count(&self) -> usize {
        self.edges.welded_count()
    }

    /// Every distinct welded edge of the part.
    pub fn edges(&self) -> &[Edge] {
        self.edges.edges()
    }

    /// The edge ids of triangle `tri` (edge `k` from corner `k` to `k + 1`); `None` for a bad triangle, and `None`
    /// per edge whose two corners weld into one.
    pub fn triangle_edges(&self, tri: usize) -> Option<[Option<u32>; 3]> {
        self.edges.triangle(tri)
    }

    /// The triangle across edge `k` of `tri`, when exactly two triangles share that edge.
    pub fn neighbour(&self, tri: usize, k: usize) -> Option<u32> {
        let id = (*self.edges.triangle(tri)?.get(k)?)?;
        self.edges.edges().get(id as usize)?.other(tri as u32)
    }

    /// UV islands: triangles joined through edges whose two sides have the same UVs.
    pub fn islands(&self) -> &Components {
        &self.islands
    }

    /// Connected pieces: triangles joined through any shared edge, regardless of UVs.
    pub fn pieces(&self) -> &Components {
        &self.pieces
    }

    /// The UV edges where islands end (borders, seams, non-manifold edges), grouped by island.
    pub fn uv_edges(&self) -> &UvEdges {
        &self.uv_edges
    }
}
