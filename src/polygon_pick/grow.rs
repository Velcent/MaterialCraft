//! [`MeshTopologies`]: grows picked triangles by the pick mode, with each part's topology built once.

use super::Picked;
use crate::mesh::{Components, Mesh, Topology};
use crate::paint::effect::PickMode;
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// The topology of every part of a mesh for one UV set, built on first use. Keep it while the mesh stays the same
/// (the editor drops it when the document's mesh changes).
pub struct MeshTopologies {
    uv_set: usize,
    parts: Vec<OnceLock<Topology>>,
}

impl MeshTopologies {
    /// Nothing built yet; islands come from UV set `uv_set` (the texture set's).
    pub fn new(mesh: &Mesh, uv_set: usize) -> MeshTopologies {
        MeshTopologies { uv_set, parts: mesh.parts.iter().map(|_| OnceLock::new()).collect() }
    }

    /// True if these topologies were made for `mesh`'s part count and UV set `uv_set`.
    pub fn fits(&self, mesh: &Mesh, uv_set: usize) -> bool {
        self.uv_set == uv_set && self.parts.len() == mesh.parts.len()
    }

    fn topology<'a>(&'a self, mesh: &Mesh, part: usize) -> Option<&'a Topology> {
        let p = mesh.parts.get(part)?;
        Some(self.parts.get(part)?.get_or_init(|| Topology::of_part(p, self.uv_set)))
    }

    /// Every picked triangle grown to what `mode` covers: itself, its source face, its connected piece of mesh or
    /// its UV island. `mesh` must be the mesh these topologies were made for.
    pub fn grow(&self, mesh: &Mesh, mode: PickMode, picked: Picked) -> Picked {
        let mut out = Picked::new();
        for (pi, tris) in picked {
            let Some(part) = mesh.parts.get(pi) else { continue };
            let grown: BTreeSet<u32> = match mode {
                PickMode::Triangle => tris.into_iter().filter(|t| (*t as usize) < part.triangles.len()).collect(),
                PickMode::Polygon => tris.iter().flat_map(|t| part.face_triangles(*t as usize)).map(|t| t as u32).collect(),
                PickMode::Mesh => self.topology(mesh, pi).map_or_else(BTreeSet::new, |tp| whole(tp.pieces(), &tris)),
                PickMode::UvIsland => self.topology(mesh, pi).map_or_else(BTreeSet::new, |tp| whole(tp.islands(), &tris)),
            };
            if !grown.is_empty() {
                out.insert(pi, grown);
            }
        }
        out
    }
}

/// Every triangle of the components the picked triangles belong to.
fn whole(components: &Components, tris: &BTreeSet<u32>) -> BTreeSet<u32> {
    let groups: BTreeSet<u32> = tris.iter().filter_map(|t| components.of(*t as usize)).collect();
    groups.into_iter().flat_map(|g| components.triangles(g)).map(|t| t as u32).collect()
}
