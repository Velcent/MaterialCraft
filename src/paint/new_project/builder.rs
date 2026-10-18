//! [`ProjectBuilder`]: a new [`Document`] from a read mesh and the [`NewProject`] settings: one texture set per
//! material made from the template's set defaults with one empty paint layer, the template's view with the file's
//! cameras, its shader instance on every set, the mesh unit, the project settings and the baked map files.

use super::baked_files;
use super::baked_names::{self, Assignment};
use super::error::NewProjectError;
use super::material_sets::{MaterialSets, PlannedSet};
use super::mesh_file::MeshSource;
use super::settings::{NewProject, UnitScale};
use crate::paint::Document;
use crate::paint::TextureSet;
use crate::paint::camera_import::imported_cameras;
use crate::paint::edit::Transaction;
use crate::paint::edit::ops::{NewLayer, create as create_layer};
use crate::paint::saved_view::SavedView;

/// Source id of cameras read from glTF files (one of `CameraOrigin::SOURCES`).
const GLTF_CAMERAS: &str = "gltf";

/// A new document and what the user should know about how it was made.
#[derive(Debug, Clone, PartialEq)]
pub struct Created {
    pub document: Document,
    /// One line each: files left out, the unit, cameras found.
    pub notes: Vec<String>,
}

/// Makes the document for one [`NewProject`].
pub struct ProjectBuilder<'a> {
    settings: &'a NewProject,
}

/// Reads the mesh file and the baked map files of `settings` and makes the document. `progress` gets a fraction 0..1
/// and a short message.
pub fn create(settings: &NewProject, progress: &mut dyn FnMut(f32, &str)) -> Result<Created, NewProjectError> {
    progress(0.0, "Reading the mesh");
    let cameras = settings.import_cameras && super::MeshFormat::of(&settings.mesh).is_some_and(|f| f.has_cameras());
    let source = MeshSource::read(&settings.mesh, cameras)?;
    progress(0.6, "Making texture sets");
    let created = ProjectBuilder::new(settings).build(source)?;
    progress(1.0, "Created");
    Ok(created)
}

impl<'a> ProjectBuilder<'a> {
    pub fn new(settings: &'a NewProject) -> ProjectBuilder<'a> {
        ProjectBuilder { settings }
    }

    /// The document for `source`. Fails for a mesh without triangles and for materials without UVs; baked map files
    /// that don't match or can't be read are left out with a note.
    pub fn build(&self, source: MeshSource) -> Result<Created, NewProjectError> {
        let MeshSource { mesh, unit, cameras, mut notes } = source;
        if mesh.parts.iter().all(|p| p.triangles.is_empty()) {
            return Err(NewProjectError::Empty);
        }
        let planned = MaterialSets::of(&mesh);
        let missing = planned.without_uvs(&mesh);
        if !missing.is_empty() {
            return Err(NewProjectError::NoUvs { materials: missing });
        }
        let mut doc = Document::new(mesh);
        doc.texture_sets = planned.sets.iter().map(|p| self.texture_set(p, &doc)).collect();
        doc.project = self.settings.project_settings();
        doc.shading.mesh_unit = match self.settings.unit {
            UnitScale::FromFile => unit,
            UnitScale::Custom(u) => Some(u),
        };
        if doc.shading.mesh_unit.is_none() {
            notes.push("The mesh file states no unit: physical sizes stay unknown until a unit is set in the shader settings.".into());
        }
        self.assign_shader(&mut doc);
        doc.view = Some(self.view(&doc, &cameras, &mut notes));
        add_paint_layers(&mut doc)?;
        self.add_baked_maps(&mut doc, &planned.sets, &mut notes);
        if let Some(problem) = doc.problems().into_iter().next() {
            return Err(NewProjectError::Invalid(problem.to_string()));
        }
        Ok(Created { document: doc, notes })
    }

    /// The template's set defaults for material `p`, at the chosen resolution.
    fn texture_set(&self, p: &PlannedSet, doc: &Document) -> TextureSet {
        let template = &self.settings.template.set;
        let size = self.settings.texture_size();
        let mut set = TextureSet::new(p.id.clone(), p.name.clone(), (size, size));
        set.material_slot = p.slot;
        set.channels = template.channels.clone();
        set.bake = template.bake.clone();
        // a template baking at its texture size bakes at the new one
        if set.bake.common.size == template.resolution.0.max(template.resolution.1) {
            set.bake.common.size = size;
        }
        let parts_have = |uv: usize| doc.mesh.parts.iter().filter(|q| q.material == p.slot).all(|q| uv < q.uv_sets.len());
        set.uv_set = if parts_have(template.uv_set) { template.uv_set } else { 0 };
        set
    }

    /// The template's shader instance on every set.
    fn assign_shader(&self, doc: &mut Document) {
        let Some(instance) = &self.settings.template.shader else { return };
        doc.shading.instances.push(instance.clone());
        for set in &doc.texture_sets {
            doc.shading.assign(&set.id, Some(instance.id.clone()));
        }
    }

    /// The template's exposure and environment with the default camera, then the file's cameras.
    fn view(&self, doc: &Document, cameras: &[crate::camera::SceneCamera], notes: &mut Vec<String>) -> SavedView {
        let mut view = SavedView { cameras: SavedView::default().cameras, active_camera: 0, ..self.settings.template.view.clone() };
        if self.settings.import_cameras && !cameras.is_empty() {
            let imported = imported_cameras(&doc.mesh, cameras, GLTF_CAMERAS);
            let left_out = cameras.len() - imported.len();
            notes.push(match left_out {
                0 => format!("Imported {} camera(s) from the mesh file.", imported.len()),
                n => format!("Imported {} camera(s) from the mesh file; {n} with an unusable pose were left out.", imported.len()),
            });
            view.cameras.extend(imported);
        }
        view
    }

    /// Reads the baked map files into the sets their names give.
    fn add_baked_maps(&self, doc: &mut Document, sets: &[PlannedSet], notes: &mut Vec<String>) {
        let Assignment { assigned, unassigned } = baked_names::assign(&self.settings.baked_maps, sets);
        for u in unassigned {
            notes.push(format!("{} was not used: {}.", u.file.display(), u.reason));
        }
        let format = self.settings.normal_map_format;
        for a in assigned {
            match baked_files::read(&a.file, &a.kind, format) {
                Ok(map) => {
                    if let Some(set) = doc.texture_set_mut(&a.set) {
                        set.baked.push(map);
                    }
                }
                Err(e) => notes.push(format!("{} could not be read: {e}.", a.file.display())),
            }
        }
    }
}

/// One empty paint layer on every set, as a new layer of the layer panel would be.
fn add_paint_layers(doc: &mut Document) -> Result<(), NewProjectError> {
    let ids: Vec<String> = doc.texture_sets.iter().map(|s| s.id.clone()).collect();
    let mut tx = Transaction::new(doc);
    for id in ids {
        create_layer(&mut tx, &id, NewLayer::Paint, None).map_err(|e| NewProjectError::Invalid(e.to_string()))?;
    }
    tx.commit();
    Ok(())
}
