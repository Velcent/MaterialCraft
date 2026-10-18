//! [`ViewPicking`]: keeps the CPU view projector's per-mesh half ([`ViewScene`]: a BVH per mesh part) in step with
//! the open document, building it on a background thread, so tools can map between the 3D view and the mesh
//! every frame ([`ViewPicking::projector`]).

use super::document_state::OpenDocument;
use super::job::Job;
use materialcraft::camera::Camera;
use materialcraft::mesh::Mesh;
use materialcraft::paint::Document;
use materialcraft::projection::{CpuViewProjector, ViewScene};
use std::path::PathBuf;

/// What a scene was built for: the document's file, the mesh's shape (part, vertex and triangle counts) and which
/// texture sets are shown.
#[derive(Clone, PartialEq, Debug)]
struct SceneKey {
    source: PathBuf,
    triangles: Vec<usize>,
    vertices: Vec<usize>,
    visible: Vec<bool>,
}

impl SceneKey {
    fn of(doc: &OpenDocument) -> SceneKey {
        let parts = &doc.doc().mesh.parts;
        SceneKey {
            source: doc.source().to_path_buf(),
            triangles: parts.iter().map(|p| p.triangles.len()).collect(),
            vertices: parts.iter().map(|p| p.positions.len()).collect(),
            visible: doc.visibility(),
        }
    }
}

#[derive(Default)]
pub struct ViewPicking {
    scene: Option<ViewScene>,
    building: Option<Job<ViewScene>>,
    key: Option<SceneKey>,
}

impl ViewPicking {
    /// Starts a rebuild when the document's mesh or shown parts changed; takes a finished build. Call once per
    /// frame.
    pub fn poll(&mut self, doc: Option<&OpenDocument>) {
        let Some(doc) = doc else {
            *self = ViewPicking::default();
            return;
        };
        let key = SceneKey::of(doc);
        if self.key.as_ref() != Some(&key) {
            let (shared, visible) = (doc.mesh_view(), key.visible.clone());
            self.building = Some(Job::spawn(move |_, tx| {
                let _ = tx.send(build(&shared, &visible));
            }));
            self.scene = None;
            self.key = Some(key);
        }
        if let Some(job) = &mut self.building {
            if let Some(scene) = job.poll().pop() {
                self.scene = Some(scene);
            }
            if job.is_finished() {
                self.building = None;
            }
        }
    }

    /// True once the scene for the current document is built.
    pub fn is_ready(&self) -> bool {
        self.scene.is_some()
    }

    /// Screen ↔ mesh through `camera` for `doc` (the document polled last), or `None` while the scene is built.
    pub fn projector<'a>(&'a self, doc: &'a Document, camera: Camera) -> Option<CpuViewProjector<'a>> {
        self.projector_on(&doc.mesh, camera)
    }

    /// [`Self::projector`] given the document's mesh alone (a brush stroke holds the texture sets meanwhile).
    pub fn projector_on<'a>(&'a self, mesh: &'a Mesh, camera: Camera) -> Option<CpuViewProjector<'a>> {
        self.scene.as_ref()?.projector(mesh, camera)
    }

    /// The picking scene, once built (replays of recorded strokes pick through it).
    pub fn scene(&self) -> Option<&ViewScene> {
        self.scene.as_ref()
    }

    /// False for mesh parts of hidden texture sets (they neither show nor hide anything).
    pub fn is_shown(&self, part: usize) -> bool {
        self.scene.as_ref().is_some_and(|s| s.is_shown(part))
    }
}

/// The scene as the viewport shows it: placed with the model's fit, each part picking in the UV set of its texture
/// set, parts of hidden texture sets left out (as `geometry::build` does).
fn build(doc: &Document, visible: &[bool]) -> ViewScene {
    let mut scene = ViewScene::new(&doc.mesh, doc.mesh.fit());
    for (i, part) in doc.mesh.parts.iter().enumerate() {
        let set = doc.texture_sets.iter().position(|s| s.material_slot == part.material);
        if let Some(s) = set {
            scene.set_uv_set(i, doc.texture_sets.get(s).map_or(0, |t| t.uv_set));
            scene.set_shown(i, visible.get(s).copied().unwrap_or(true));
        }
    }
    scene
}
