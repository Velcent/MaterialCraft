//! [`UvOutlines`]: the UV island outlines of the texture set the 2D view shows (V2-04), built once per mesh and set
//! on a background thread from `mesh::topology`'s UV edge lists.

use super::job::Job;
use super::shared_mesh::SharedMesh;
use materialcraft::mesh::Mesh;
use materialcraft::mesh::Topology;
use std::sync::Arc;

/// The outline edges of one texture set's parts, in UV units (v down).
#[derive(Debug, Default, PartialEq)]
pub struct Outline {
    pub edges: Vec<[[f32; 2]; 2]>,
}

/// The outline of the parts of `mesh` with material slot `material`, through UV set `uv_set`: every edge where an
/// island ends (borders, seams from each side, non-manifold edges).
pub fn outline_of(mesh: &Mesh, material: usize, uv_set: usize) -> Outline {
    let mut edges = Vec::new();
    for part in mesh.parts.iter().filter(|p| p.material == material && p.uv_sets.get(uv_set).is_some()) {
        let topology = Topology::of_part(part, uv_set);
        edges.extend(topology.uv_edges().all().iter().map(|e| e.uv));
    }
    Outline { edges }
}

/// Which outline is wanted: the mesh revision, the material slot and the UV set.
type Key = (u64, usize, usize);

/// The outline of the set the 2D view shows, kept until the mesh or the set changes.
#[derive(Default)]
pub struct UvOutlines {
    key: Option<Key>,
    job: Option<Job<Outline>>,
    ready: Option<Arc<Outline>>,
}

impl UvOutlines {
    /// The outline for `key` once built (starts building it on a background thread when the key changed). Call once
    /// per frame.
    pub fn get(&mut self, mesh: SharedMesh, material: usize, uv_set: usize) -> Option<Arc<Outline>> {
        let key = (mesh.revision, material, uv_set);
        let shared = mesh.mesh;
        if self.key != Some(key) {
            self.key = Some(key);
            self.ready = None;
            if let Some(old) = self.job.take() {
                old.cancel();
            }
            self.job = Some(Job::spawn(move |_, tx| {
                let _ = tx.send(outline_of(&shared, material, uv_set));
            }));
        }
        if let Some(job) = &mut self.job {
            if let Some(outline) = job.poll().pop() {
                self.ready = Some(Arc::new(outline));
            }
            if job.is_finished() {
                self.job = None;
            }
        }
        self.ready.clone()
    }

    /// True while an outline is being built.
    pub fn is_building(&self) -> bool {
        self.job.is_some()
    }

    /// Forgets the document's outline.
    pub fn clear(&mut self) {
        *self = UvOutlines::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::mesh::MeshPart;

    /// Two triangles sharing an edge in 3D; `split` gives the second its own UV island.
    fn quad(split: bool) -> Mesh {
        let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 0.0]];
        let shift = if split { 0.5 } else { 0.0 };
        let uv = vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0 + shift, 0.0], [0.0 + shift, 1.0], [1.0 + shift, 0.0]];
        let part = MeshPart { positions, triangles: vec![[0, 1, 2], [4, 5, 3]], uv_sets: vec![uv], ..MeshPart::default() };
        Mesh { parts: vec![part], ..Mesh::default() }
    }

    #[test]
    fn outlines_follow_islands_and_skip_other_materials() {
        // one island: the four borders, the shared diagonal is no outline
        assert_eq!(outline_of(&quad(false), 0, 0).edges.len(), 4);
        // two islands: the diagonal is a seam seen from both sides
        assert_eq!(outline_of(&quad(true), 0, 0).edges.len(), 6);
        assert!(outline_of(&quad(false), 1, 0).edges.is_empty(), "another material slot");
        assert!(outline_of(&quad(false), 0, 2).edges.is_empty(), "no such UV set");
    }
}
