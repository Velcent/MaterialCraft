//! [`SurfaceCache`]: the surface map and texel index of the texture set being painted, built on a background thread
//! as soon as a paintable target is selected, so the first stroke does not wait. One set is kept (a 4K map takes
//! about 270 MB).

use super::super::document_state::OpenDocument;
use super::super::job::Job;
use materialcraft::mesh::{SurfaceKey, SurfaceMap, mesh_fingerprint};
use materialcraft::stamp::TexelIndex;
use std::sync::Arc;

/// A texture set's surface map and its index.
pub struct Surface {
    pub map: SurfaceMap,
    pub index: TexelIndex,
}

/// What a surface was built for: the set and its placement, and the mesh's shape (part and triangle counts).
#[derive(Debug, Clone, PartialEq)]
struct Wanted {
    set: String,
    material: usize,
    uv_set: usize,
    size: (u32, u32),
    triangles: Vec<usize>,
}

impl Wanted {
    fn of(doc: &OpenDocument, set: usize) -> Option<Wanted> {
        let d = doc.doc();
        let s = d.texture_sets.get(set)?;
        Some(Wanted {
            set: s.id.clone(),
            material: s.material_slot,
            uv_set: s.uv_set,
            size: s.resolution,
            triangles: d.mesh.parts.iter().map(|p| p.triangles.len()).collect(),
        })
    }
}

#[derive(Default)]
pub struct SurfaceCache {
    ready: Option<(Wanted, Arc<Surface>)>,
    building: Option<(Wanted, Job<Result<Surface, String>>)>,
    /// Why the last build failed (reported once).
    pub error: Option<String>,
}

impl SurfaceCache {
    /// The surface of texture set `set`, once built; starts building it otherwise.
    pub fn get(&mut self, doc: &OpenDocument, set: usize) -> Option<Arc<Surface>> {
        let wanted = Wanted::of(doc, set)?;
        self.poll();
        if let Some((w, s)) = &self.ready
            && *w == wanted
        {
            return Some(s.clone());
        }
        if self.building.as_ref().is_none_or(|(w, _)| *w != wanted) {
            let view = doc.mesh_view();
            let (material, uv_set, size) = (wanted.material, wanted.uv_set, wanted.size);
            let job = Job::spawn(move |_, tx| {
                let key = SurfaceKey { material, uv_set, width: size.0, height: size.1, gutter: 8, mesh_revision: mesh_fingerprint(&view.mesh, material) };
                let built = SurfaceMap::build(&view.mesh, key).map(|map| Surface { index: TexelIndex::build(&map), map }).map_err(|e| e.to_string());
                let _ = tx.send(built);
            });
            self.building = Some((wanted, job));
        }
        None
    }

    /// True while a surface is being built.
    pub fn is_building(&self) -> bool {
        self.building.is_some()
    }

    /// Takes a finished build. Call once per frame.
    pub fn poll(&mut self) {
        let Some((wanted, job)) = &mut self.building else { return };
        if let Some(result) = job.poll().pop() {
            match result {
                Ok(s) => self.ready = Some((wanted.clone(), Arc::new(s))),
                Err(e) => self.error = Some(e),
            }
        }
        if job.is_finished() {
            self.building = None;
        }
    }

    /// Forgets everything (another document).
    pub fn clear(&mut self) {
        *self = SurfaceCache::default();
    }
}
