//! [`material_uids`]: how a texture set finds its baking settings. `BakingMaterial.materialUID` refers to the
//! `DataMaterial` object's uid in `document.bin`, while sets are bound by `sceneMaterialUID`.

use crate::model::RawTree;
use crate::project::{SppProject, paths};
use std::collections::BTreeMap;

/// `sceneMaterialUID` → `DataMaterial` uid, from the document's object tree (empty if unreadable).
pub fn material_uids(project: &SppProject) -> BTreeMap<u64, u64> {
    let Ok(data) = project.read(paths::DOCUMENT) else {
        return BTreeMap::new();
    };
    let Ok(tree) = RawTree::parse(&data) else {
        return BTreeMap::new();
    };
    tree.root
        .iter()
        .flat_map(|root| root.objects("materials"))
        .filter_map(|m| Some((m.u64("sceneMaterialUID")?, m.uid()?)))
        .collect()
}
