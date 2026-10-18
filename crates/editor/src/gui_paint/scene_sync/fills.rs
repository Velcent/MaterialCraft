//! Where the scene's maps runs get their fill sources: the surface maps fills composite with and the resolver of
//! material fill references.

use super::SceneSync;
use crate::gui_paint::document_state::OpenDocument;
use crate::gui_paint::fill_surfaces::FillSurfaces;
use materialcraft::material_fill::{ChainResolver, FolderResolver, MaterialFills, MaterialResolver, MemoryResolver};
use std::path::PathBuf;
use std::sync::Arc;

impl SceneSync {
    /// The surface maps the sets' fills composite with, when one of them needs its surface: the kept ones while the
    /// mesh is the same, else new (empty) ones for the current mesh.
    pub(super) fn fill_surfaces_for(&mut self, doc: &mut OpenDocument, sets: &[usize]) -> Option<Arc<FillSurfaces>> {
        let needed = sets.iter().any(|&i| doc.doc().texture_sets.get(i).is_some_and(|s| s.needs_surface()));
        if !needed {
            return self.surfaces.clone();
        }
        let mesh = doc.shared_mesh();
        if self.surfaces.as_ref().is_none_or(|s| s.revision() != mesh.revision) {
            self.surfaces = Some(Arc::new(FillSurfaces::new(mesh)));
        }
        self.surfaces.clone()
    }

    /// The surface maps of the view's fills (the eyedropper and the layer thumbnails composite with them too).
    pub fn fill_surfaces(&self) -> Option<Arc<FillSurfaces>> {
        self.surfaces.clone()
    }

    /// How material fills of `doc` are evaluated, as the maps the view shows evaluate them (the eyedropper's
    /// stored-value pick composites through the same).
    pub fn material_fills(&self, doc: &OpenDocument) -> MaterialFills {
        MaterialFills::new(Arc::new(material_resolver(doc, &self.library_folders)), self.materials.clone())
    }

    /// Sets the asset library roots material references are looked up in (used by the next maps run).
    pub fn set_library_folders(&mut self, folders: Vec<PathBuf>) {
        self.library_folders = folders;
    }
}

/// Where material fill references are looked up: the document's embedded assets (AS-21, so it looks the same
/// without the library), then its folder, then the asset library roots.
fn material_resolver(doc: &OpenDocument, library: &[PathBuf]) -> ChainResolver {
    let folders = FolderResolver::new(doc.source().parent().map(|p| p.to_path_buf()).into_iter().chain(library.iter().cloned()));
    ChainResolver::new([Arc::new(MemoryResolver::from_embedded(&doc.doc().assets)) as Arc<dyn MaterialResolver>, Arc::new(folders)])
}
