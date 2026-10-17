//! [`MeshShare`]: one shared copy of the open document's mesh for background workers that keep data derived from it
//! (surface maps for fills), with a revision that changes whenever the mesh is replaced. The copy is taken once
//! and handed out again until the document's mesh changes, so a recomposite after an edit copies no mesh.

use materialcraft::mesh::Mesh;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Revisions are unique across documents, so data keyed by one never matches another document's mesh.
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

/// A copy of the mesh and its revision.
#[derive(Clone)]
pub struct SharedMesh {
    pub mesh: Arc<Mesh>,
    pub revision: u64,
}

/// What identifies a mesh without reading its vertices: its part buffers' addresses and lengths. The mesh only changes
/// by being replaced as a whole (`Edit::SetMesh`, undo and redo swap meshes), and both meshes are alive while they are
/// swapped, so a replaced mesh never has the old one's buffers.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp(Vec<(usize, usize)>);

impl Stamp {
    fn of(mesh: &Mesh) -> Stamp {
        let mut v = vec![(mesh.parts.as_ptr() as usize, mesh.parts.len())];
        for p in &mesh.parts {
            v.push((p.positions.as_ptr() as usize, p.positions.len()));
            v.push((p.triangles.as_ptr() as usize, p.triangles.len()));
        }
        Stamp(v)
    }
}

#[derive(Default)]
pub struct MeshShare {
    current: Option<(Stamp, SharedMesh)>,
}

impl MeshShare {
    /// The shared copy of `mesh`; copies it only when it changed since the last call.
    pub fn get(&mut self, mesh: &Mesh) -> SharedMesh {
        let stamp = Stamp::of(mesh);
        if let Some((s, shared)) = &self.current
            && *s == stamp
        {
            return shared.clone();
        }
        let shared = SharedMesh { mesh: Arc::new(mesh.clone()), revision: NEXT_REVISION.fetch_add(1, Ordering::Relaxed) };
        self.current = Some((stamp, shared.clone()));
        shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::mesh::MeshPart;

    fn mesh(n: usize) -> Mesh {
        Mesh { parts: vec![MeshPart { positions: vec![[0.0; 3]; n], triangles: vec![[0, 1, 2]], ..MeshPart::default() }], ..Mesh::default() }
    }

    #[test]
    fn copies_once_per_mesh() {
        let (mut share, mut m) = (MeshShare::default(), mesh(3));
        let a = share.get(&m);
        let b = share.get(&m);
        assert!(Arc::ptr_eq(&a.mesh, &b.mesh) && a.revision == b.revision, "an unchanged mesh is not copied again");
        let old = std::mem::replace(&mut m, mesh(4));
        let c = share.get(&m);
        assert_ne!(c.revision, a.revision, "a replaced mesh gets a new revision");
        let _ = std::mem::replace(&mut m, old);
        assert_ne!(share.get(&m).revision, c.revision, "so does swapping the old one back (undo)");
    }
}
