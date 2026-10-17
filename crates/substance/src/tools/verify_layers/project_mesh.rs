//! [`ProjectMesh`]: a project's mesh with the Painter material uid of each slot, so a texture set's texels on
//! the mesh ([`IslandTexels`]) can be found for its cache score.

use crate::decode::assbin::{AssbinGeometry, AssbinScene};
use crate::project::{SppProject, paths};
use crate::translate::layers::island_texels::IslandTexels;
use materialcraft::mesh::{Mesh, SurfaceKey, SurfaceMap};

pub struct ProjectMesh {
    mesh: Mesh,
    /// Per material slot.
    uids: Vec<Option<u64>>,
}

impl ProjectMesh {
    /// `None` when the mesh cannot be read (scores then compare every texel).
    pub fn read(project: &SppProject) -> Option<ProjectMesh> {
        let scene = project.read(paths::SCENE).ok()?;
        let mesh = AssbinGeometry::parse(&scene).ok()?;
        let uids = AssbinScene::parse(&scene)
            .ok()?
            .materials
            .iter()
            .map(|m| m.uid)
            .collect();
        Some(ProjectMesh { mesh, uids })
    }

    /// The surface map of the first slot with material `uid` at `size`.
    pub fn surface(&self, uid: u64, size: (u32, u32)) -> Option<SurfaceMap> {
        let material = (0..self.uids.len()).find(|&i| self.uids[i] == Some(uid))?;
        let key = SurfaceKey {
            material,
            uv_set: 0,
            width: size.0,
            height: size.1,
            gutter: 0,
            mesh_revision: 0,
        };
        SurfaceMap::build(&self.mesh, key).ok()
    }

    /// The texels of the set with material `uid` (all slots Painter collapsed into it) at `size`.
    pub fn islands(&self, uid: u64, size: (u32, u32)) -> Option<IslandTexels> {
        let slots: Vec<usize> = (0..self.uids.len()).filter(|&i| self.uids[i] == Some(uid)).collect();
        IslandTexels::rasterise(&self.mesh, &slots, 0, size)
    }
}
