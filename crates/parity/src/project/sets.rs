//! [`SetCase`]: one texture set of a project as a parity case: its name, mesh slots, baked-map references and
//! the uid that links it to its baking settings.

use materialcraft_substance::decode::assbin::AssbinScene;
use materialcraft_substance::decode::document::BitmapRef;
use materialcraft_substance::project::SppProject;
use materialcraft_substance::translate::SetBinding;
use materialcraft_substance::translate::bake_settings::material_uids;

#[derive(Debug, Clone, PartialEq)]
pub struct SetCase {
    /// The set name Painter shows (and names its baked maps after).
    pub name: String,
    /// Mesh material slots painted by this set (Painter can collapse several into one).
    pub slots: Vec<usize>,
    /// `DataMaterial` uid in `document.bin`; `BakingMaterial.materialUID` refers to it.
    pub material_uid: Option<u64>,
    /// The document's baked-map references (empty: resource names are used).
    pub refs: Vec<BitmapRef>,
}

/// The project's texture sets, bound to mesh slots the way the importer binds them.
pub fn discover(project: &SppProject, scene: &[u8]) -> Result<Vec<SetCase>, String> {
    let materials = AssbinScene::parse(scene).map_err(|e| format!("mesh: {e}"))?.materials;
    let records = project.document_materials().map_err(|e| e.to_string())?;
    let uids = material_uids(project);
    let binding = SetBinding::bind(&materials, &records, &Default::default());
    Ok(binding
        .sets
        .into_iter()
        .map(|s| SetCase {
            name: s.record.name.clone(),
            slots: std::iter::once(s.slot).chain(s.merged_slots.iter().copied()).collect(),
            material_uid: s.record.scene_uid.and_then(|u| uids.get(&u).copied()),
            refs: s.record.baked.clone(),
        })
        .collect())
}
