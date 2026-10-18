//! [`ProjectBaking`]: everything a project's texture sets need to get their bake configuration: Painter's
//! settings, the uid link to them and the normalisation that moves high-poly files into the document's space.

use super::{MappedBake, material_uids};
use crate::decode::document::MaterialRecord;
use crate::model::baking::{BAKING_INI, MaterialBaking, PainterBaking};
use crate::project::SppProject;
use materialcraft::paint::bake_config::{HighPlacement, SetBakeConfig};
use std::collections::BTreeMap;

pub struct ProjectBaking {
    baking: Option<PainterBaking>,
    /// `sceneMaterialUID` → `DataMaterial` uid.
    uids: BTreeMap<u64, u64>,
    placement: Option<HighPlacement>,
}

impl ProjectBaking {
    /// Reads the settings of `project`; `scene` is its `scene.assbin`. Missing or unreadable settings leave every
    /// set with our defaults (said in [`Self::config`]'s notes).
    pub fn read(project: &SppProject, scene: &[u8]) -> ProjectBaking {
        let baking = project
            .read(BAKING_INI)
            .ok()
            .and_then(|data| PainterBaking::parse(&data).ok());
        let placement = project.mesh_normalization(scene).ok().flatten().map(|n| HighPlacement {
            center: n.center,
            radius: n.radius,
        });
        ProjectBaking {
            baking,
            uids: material_uids(project),
            placement,
        }
    }

    fn settings_of(&self, record: &MaterialRecord) -> Option<&MaterialBaking> {
        let uid = record.scene_uid.and_then(|u| self.uids.get(&u).copied());
        self.baking.as_ref()?.for_material(uid)
    }

    /// The configuration of the set `record`, at Painter's output size (else `resolution`), and what of
    /// Painter's settings it could not take over.
    pub fn config(&self, record: &MaterialRecord, resolution: (u32, u32)) -> (SetBakeConfig, Vec<String>) {
        let painter = self.settings_of(record);
        let size = painter
            .and_then(output_size)
            .unwrap_or(resolution.0.max(resolution.1).max(1) as usize);
        let mapped = MappedBake::of(painter, size);
        let notes = mapped.unmapped.iter().map(|u| u.text.clone()).collect();
        (mapped.config(self.placement), notes)
    }
}

/// `Common.OutputSize`: log2 of width and height; ours is square, so the larger.
fn output_size(p: &MaterialBaking) -> Option<usize> {
    match p.common.get("Common.OutputSize")? {
        crate::model::baking::TweakValue::Int2([w, h]) => {
            let log2 = (*w).max(*h);
            (0..=14).contains(&log2).then(|| 1usize << log2)
        }
        _ => None,
    }
}
