//! [`FillSurfaces`]: the surface maps and the scene frame that fills need to composite (polygon fills, and fills
//! projected from a volume, see [`TextureSet::needs_surface`]), kept across maps runs. A set's map is built on a
//! worker the first time it is asked for and reused until the set's placement or resolution or the mesh changes; the
//! UI thread only reads what is already built (the eyedropper), it never builds.

use super::shared_mesh::SharedMesh;
use materialcraft::mesh::{SurfaceKey, SurfaceMap};
use materialcraft::paint::{FillContext, FillSources, TextureSet};
use materialcraft::projection::SceneFrame;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Sizes kept per set placement: the set's resolution, the isolated mask's grid and the layer thumbnails' grid.
const SIZES_PER_PLACEMENT: usize = 3;

/// A set's surface map, or `None` if it could not be built (its UV set is missing); built once either way.
type Built = Option<Arc<SurfaceMap>>;

/// Surface maps of one mesh revision.
pub struct FillSurfaces {
    mesh: SharedMesh,
    frame: OnceLock<Option<SceneFrame>>,
    /// Each map with the order it was stored in (older sizes of a placement are dropped).
    maps: Mutex<(HashMap<SurfaceKey, (Built, u64)>, u64)>,
}

/// A surface map with the frame it is placed in, which a [`FillContext`] borrows.
pub struct SetSurface {
    pub map: Arc<SurfaceMap>,
    pub frame: Option<SceneFrame>,
}

impl SetSurface {
    /// `sources` with this surface and frame.
    pub fn context<'a>(&'a self, sources: &'a dyn FillSources) -> FillContext<'a> {
        FillContext::with_sources(sources).with_surface(&self.map, self.frame)
    }
}

impl FillSurfaces {
    pub fn new(mesh: SharedMesh) -> FillSurfaces {
        FillSurfaces { mesh, frame: OnceLock::new(), maps: Mutex::new((HashMap::new(), 0)) }
    }

    /// The mesh revision the maps are built from.
    pub fn revision(&self) -> u64 {
        self.mesh.revision
    }

    /// The surface of `set`'s placement at `size` texels, built now if it is not yet (call off the UI thread). `None`
    /// when the mesh has no surface for it.
    pub fn build(&self, set: &TextureSet, size: (u32, u32)) -> Option<SetSurface> {
        let key = self.key(set, size);
        let map = match self.lookup(key) {
            Some(built) => built,
            None => {
                // built outside the lock, so the UI thread's lookups never wait for a build
                let built = SurfaceMap::build(&self.mesh.mesh, key).ok().map(Arc::new);
                self.store(key, built.clone());
                built
            }
        }?;
        Some(SetSurface { map, frame: self.frame() })
    }

    /// The surface of `set`'s placement at `size`, if it is already built (never builds: safe on the UI thread).
    pub fn cached(&self, set: &TextureSet, size: (u32, u32)) -> Option<SetSurface> {
        let map = self.lookup(self.key(set, size)).flatten()?;
        Some(SetSurface { map, frame: self.frame.get().copied().flatten() })
    }

    /// The frame of the whole scene, which volume projections are placed in.
    fn frame(&self) -> Option<SceneFrame> {
        *self.frame.get_or_init(|| SceneFrame::of_mesh(&self.mesh.mesh))
    }

    fn key(&self, set: &TextureSet, (width, height): (u32, u32)) -> SurfaceKey {
        SurfaceKey { material: set.material_slot, uv_set: set.uv_set, width, height, gutter: 0, mesh_revision: self.mesh.revision }
    }

    /// `Some(built)` if `key` was built (successfully or not), `None` if it was not tried yet.
    fn lookup(&self, key: SurfaceKey) -> Option<Built> {
        let maps = self.maps.lock().ok()?;
        maps.0.get(&key).map(|(b, _)| b.clone())
    }

    /// Keeps `built`, dropping the oldest size of its placement beyond [`SIZES_PER_PLACEMENT`] (a changed resolution).
    fn store(&self, key: SurfaceKey, built: Built) {
        let Ok(mut guard) = self.maps.lock() else { return };
        let (maps, next) = &mut *guard;
        *next += 1;
        maps.insert(key, (built, *next));
        let same = |k: &SurfaceKey| (k.material, k.uv_set) == (key.material, key.uv_set);
        while maps.keys().filter(|k| same(k)).count() > SIZES_PER_PLACEMENT {
            let Some(oldest) = maps.iter().filter(|(k, _)| same(k)).min_by_key(|(_, (_, n))| *n).map(|(k, _)| *k) else { break };
            maps.remove(&oldest);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_paint::shared_mesh::MeshShare;
    use materialcraft::mesh::{Mesh, MeshPart};

    fn surfaces() -> FillSurfaces {
        let part = MeshPart {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uv_sets: vec![vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]],
            triangles: vec![[0, 1, 2]],
            ..MeshPart::default()
        };
        FillSurfaces::new(MeshShare::default().get(&Mesh { materials: vec!["m".into()], parts: vec![part] }))
    }

    #[test]
    fn maps_are_built_once_and_older_sizes_dropped() {
        let (s, set) = (surfaces(), TextureSet::new("s", "S", (16, 16)));
        assert!(s.cached(&set, (16, 16)).is_none(), "nothing is built by looking");
        let a = s.build(&set, (16, 16)).expect("built");
        assert!(a.frame.is_some());
        assert!(Arc::ptr_eq(&a.map, &s.build(&set, (16, 16)).expect("kept").map));
        assert!(Arc::ptr_eq(&a.map, &s.cached(&set, (16, 16)).expect("kept").map));
        s.build(&set, (8, 8)).expect("thumbnail size");
        s.build(&set, (12, 12)).expect("isolated mask size");
        assert!(s.cached(&set, (16, 16)).is_some(), "three sizes per placement are kept");
        s.build(&set, (32, 32)).expect("new resolution");
        assert!(s.cached(&set, (16, 16)).is_none(), "the oldest size is dropped");
        assert!([(8, 8), (12, 12), (32, 32)].iter().all(|&size| s.cached(&set, size).is_some()));
        let mut other_uv = set.clone();
        other_uv.uv_set = 3;
        assert!(s.build(&other_uv, (16, 16)).is_none(), "a missing UV set gives no surface");
    }
}
