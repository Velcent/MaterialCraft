//! Smart materials and smart masks on the Paint screen: "Save as smart material" and "Save as smart mask" write a
//! fragment into the user's asset library (MK-15), where the tray picks it up on its next poll; a dropped one whose
//! baked maps the texture set lacks starts a bake of exactly those maps in the bakers' queue (AS-05). The fragment
//! itself is made by `paint::fragment::make`; this only finds the files and reports.

use super::PaintMode;
use super::panels::layers::SaveAsset;
use materialcraft::assets::{Usage, new_asset_path};
use materialcraft::bake::MapSelection;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::generators;
use materialcraft::material_fill::FolderResolver;
use materialcraft::paint::bake_config::map_kind;
use materialcraft::paint::format::fragment;
use materialcraft::paint::fragment::{make, needs};
use materialcraft::paint::{BakedKind, FragmentKind};

impl PaintMode {
    /// Saves the selection or the mask `what` names as a fragment in the user's library.
    pub(super) fn save_asset(&mut self, what: SaveAsset) {
        let Some(doc) = &self.document else { return };
        let Some(library) = self.tray.user_library().map(|p| p.to_path_buf()) else {
            self.diag.warn(Area::Assets, "There is no asset library to save into (settings are kept in memory).");
            return;
        };
        let (set_id, made) = match &what {
            SaveAsset::SmartMaterial { set, ids } => (set, doc.doc().texture_set(set).and_then(|s| make::smart_material(s, ids))),
            SaveAsset::SmartMask(r) => (&r.set, doc.doc().texture_set(&r.set).and_then(|s| make::smart_mask(s, &r.layer))),
        };
        let Some(mut f) = made else {
            self.diag.warn(Area::Assets, format!("Nothing to save in texture set {set_id:?}: select layers, or a layer with a mask."));
            return;
        };
        // Assets the layers use come from the document's embedded copies, else from the library folders.
        let embedded = &doc.doc().assets;
        let folders = FolderResolver::new(doc.source().parent().map(|p| p.to_path_buf()).into_iter().chain(self.tray.material_folders()));
        let report = f.complete(&generators::baked_maps, |r| {
            embedded.iter().find(|a| a.reference == r).map(|a| a.bytes.clone()).or_else(|| std::fs::read(folders.locate(r)?).ok())
        });
        let (usage, what) = match f.kind() {
            FragmentKind::SmartMaterial => (Usage::SmartMaterial, "smart material"),
            FragmentKind::SmartMask => (Usage::SmartMask, "smart mask"),
        };
        let path = new_asset_path(&library, usage, &f.name, fragment::EXTENSION);
        let saved = match path.parent() {
            Some(dir) => std::fs::create_dir_all(dir).map_err(|e| e.to_string()).and_then(|()| fragment::save_path(&f, &path).map_err(|e| e.to_string())),
            None => Err("no folder".to_string()),
        };
        match saved {
            Ok(()) => {
                let needs = if f.needs.is_empty() { String::new() } else { format!(" It needs these baked maps: {}.", map_names(&f.needs)) };
                self.diag.info(Area::Assets, format!("Saved {what} \"{}\" to {}.{needs}", f.name, path.display()));
                if !report.missing.is_empty() {
                    self.diag.warn(
                        Area::Assets,
                        format!("\"{}\" uses {} which could not be found; they are not embedded in it.", f.name, report.missing.join(", ")),
                    );
                }
            }
            Err(e) => self.diag.warn(Area::Assets, format!("The {what} \"{}\" was not saved to {}: {e}.", f.name, path.display())),
        }
    }

    /// After `name` (an asset needing the baked maps `needs`, by id) was dropped onto texture set `set`: bakes the
    /// maps the set lacks, and says so in the log (cancel in the Bakers panel).
    pub(super) fn bake_missing_maps(&mut self, set: &str, name: &str, needs: &[String]) {
        let Some(doc) = &self.document else { return };
        let Some(s) = doc.doc().texture_set(set) else { return };
        let wanted: Vec<BakedKind> = needs.iter().map(|id| BakedKind::from_id(id)).collect();
        let missing = needs::missing(&wanted, s);
        if missing.is_empty() {
            return;
        }
        let bakeable: Vec<_> = missing.iter().filter_map(map_kind).collect();
        let set_name = s.name.clone();
        self.diag.warn(
            Area::Bake,
            format!(
                "\"{name}\" needs {} on texture set \"{set_name}\", which it has not baked.{}",
                map_names(&missing),
                if bakeable.is_empty() { " Bake them in the Bakers panel." } else { " Baking them now; the Bakers panel can cancel." }
            ),
        );
        if !bakeable.is_empty() {
            self.start_bake(&[set.to_string()], Some(MapSelection::of(&bakeable)));
        }
    }
}

/// "ambient occlusion, curvature".
fn map_names(maps: &[BakedKind]) -> String {
    maps.iter().map(|k| k.id().replace('_', " ")).collect::<Vec<_>>().join(", ")
}
