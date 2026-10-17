//! [`BakeJob`]: one texture set to bake, built from the document's configuration.

use materialcraft::bake::{BakeSettings, MapSelection, TextureSetRef};
use materialcraft::paint::Document;
use materialcraft::paint::bake_config::HighPoly;

/// What the queue bakes for one set.
#[derive(Debug, Clone, PartialEq)]
pub struct BakeJob {
    /// Texture set id.
    pub set: String,
    /// Its name, for progress and log lines.
    pub name: String,
    pub target: TextureSetRef,
    pub settings: BakeSettings,
    pub high: HighPoly,
}

impl BakeJob {
    /// The jobs for `sets` (ids) of `doc`, in the order given: every included map, or (`maps`) just these maps.
    /// Unknown ids and sets with nothing to bake are left out.
    pub fn for_sets(doc: &Document, sets: &[String], maps: Option<MapSelection>) -> Vec<BakeJob> {
        sets.iter()
            .filter_map(|id| doc.texture_set(id))
            .map(|s| BakeJob {
                set: s.id.clone(),
                name: s.name.clone(),
                target: TextureSetRef { materials: vec![s.material_slot], uv_set: s.uv_set },
                settings: maps.map_or_else(|| s.bake.settings(), |m| s.bake.settings_for(m)),
                high: s.bake.common.high.clone(),
            })
            .filter(|j| !j.settings.maps.is_empty())
            .collect()
    }
}
