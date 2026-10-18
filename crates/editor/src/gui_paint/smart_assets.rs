//! Smart materials and smart masks on the Paint screen. "Save as smart material" and "Save as smart mask" make a
//! fragment of the selection or mask, take a thumbnail of the 3D view, ask for the name and the library
//! ([`SmartAssetDialog`]) and write it into the library's usage folder (MK-15), where the tray picks it up on its next
//! poll. A dropped one whose baked maps the texture set lacks warns and asks whether to bake them now (AS-05), unless
//! the user told it to stop asking. The fragment itself is made by `paint::fragment::make`.

use super::PaintMode;
use super::dialogs::bake_missing::BakePrompt;
use super::dialogs::smart_asset::{Library, Outcome, SmartAssetChoice, SmartAssetDialog};
use super::panels::layers::SaveAsset;
use super::view_thumbnail::ViewShot;
use crate::gui_settings::MissingMaps as MissingMapsAnswer;
use eframe::egui;
use materialcraft::assets::{AssetStore, Draft, LibraryRoot, RootKind, Usage};
use materialcraft::bake::MapKind;
use materialcraft::bake::MapSelection;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::generators;
use materialcraft::paint::bake_config::map_kind;
use materialcraft::paint::format::{SaveOptions, Thumbnail, fragment};
use materialcraft::paint::fragment::{make, needs};
use materialcraft::paint::{BakedKind, Document, Fragment, FragmentKind};
use materialcraft::tex::Tex;

/// The smart-asset questions in progress.
#[derive(Default)]
pub struct SmartAssets {
    naming: Option<Naming>,
    /// The bake question: the set's id and the maps to bake.
    bake: Option<(BakePrompt, String, Vec<MapKind>)>,
}

/// A fragment on its way into the library.
enum Naming {
    /// Made; the view's screenshot is asked for next frame (before the dialog covers the view).
    Made(Fragment),
    Capturing(Fragment, ViewShot),
    Asking(Fragment, SmartAssetDialog, Option<Tex>),
}

/// Baked maps an asset dropped onto a texture set needs and the set has not baked.
#[derive(Clone, PartialEq, Debug)]
pub struct MissingMaps {
    pub set_name: String,
    pub missing: Vec<BakedKind>,
    /// The missing maps the bakers can make.
    pub bakeable: Vec<MapKind>,
}

impl MissingMaps {
    /// The maps of `needs` (by id) that texture set `set` of `doc` lacks; `None` if it has them all.
    pub fn of(doc: &Document, set: &str, needs: &[String]) -> Option<MissingMaps> {
        let s = doc.texture_set(set)?;
        let wanted: Vec<BakedKind> = needs.iter().map(|id| BakedKind::from_id(id)).collect();
        let missing = needs::missing(&wanted, s);
        if missing.is_empty() {
            return None;
        }
        let bakeable = missing.iter().filter_map(map_kind).collect();
        Some(MissingMaps { set_name: s.name.clone(), missing, bakeable })
    }

    /// The log's warning about asset `name`.
    pub fn warning(&self, name: &str) -> String {
        format!("\"{name}\" needs {} on texture set \"{}\", which it has not baked.", map_names(&self.missing), self.set_name)
    }
}

impl PaintMode {
    /// "Save as smart material / mask": makes the fragment of the selection or mask `what` names and starts asking
    /// for its name.
    pub(super) fn save_asset(&mut self, what: SaveAsset) {
        let Some(doc) = &self.document else { return };
        if self.tray.user_library().is_none() && self.tray.material_folders().is_empty() {
            self.diag.warn(Area::Assets, "There is no asset library to save into (settings are kept in memory).");
            return;
        }
        let (set_id, made) = match &what {
            SaveAsset::SmartMaterial { set, ids } => (set, doc.doc().texture_set(set).and_then(|s| make::smart_material(s, ids))),
            SaveAsset::SmartMask(r) => (&r.set, doc.doc().texture_set(&r.set).and_then(|s| make::smart_mask(s, &r.layer))),
        };
        match made {
            Some(f) => self.smart.naming = Some(Naming::Made(f)),
            None => self.diag.warn(Area::Assets, format!("Nothing to save in texture set {set_id:?}: select layers, or a layer with a mask.")),
        }
    }

    /// Advances the naming dialog and the bake question.
    pub(super) fn poll_smart_assets(&mut self, ctx: &egui::Context) {
        self.smart.naming = match self.smart.naming.take() {
            None => None,
            Some(Naming::Made(f)) => Some(Naming::Capturing(f, ViewShot::request(ctx))),
            Some(Naming::Capturing(f, mut shot)) => match shot.poll(ctx, self.viewport.rect()) {
                None => Some(Naming::Capturing(f, shot)),
                Some(thumbnail) => {
                    let texture = thumbnail.as_ref().map(|t| ctx.load_texture("smart_asset_thumbnail", color_image(t), egui::TextureOptions::LINEAR));
                    let what = what(&f);
                    let dialog = SmartAssetDialog::new(what, f.name.clone(), self.libraries(), texture);
                    Some(Naming::Asking(f, dialog, thumbnail))
                }
            },
            Some(Naming::Asking(f, mut dialog, thumbnail)) => match dialog.show(ctx) {
                None => Some(Naming::Asking(f, dialog, thumbnail)),
                Some(Outcome::Cancel) => None,
                Some(Outcome::Save(choice)) => {
                    let thumbnail = thumbnail.filter(|_| choice.use_thumbnail);
                    self.write_fragment(f, &choice, thumbnail);
                    None
                }
            },
        };
        if let Some((prompt, _, _)) = &mut self.smart.bake
            && let Some(answer) = prompt.show(ctx)
            && let Some((_, set, maps)) = self.smart.bake.take()
        {
            if answer.remember {
                let remembered = if answer.bake { MissingMapsAnswer::Bake } else { MissingMapsAnswer::Skip };
                self.shell.settings.update(|s| s.prompts.missing_maps = remembered);
            }
            if answer.bake {
                self.start_bake(&[set], Some(MapSelection::of(&maps)));
            } else {
                self.diag.info(Area::Bake, "Not baked now; the Bakers panel bakes them any time.");
            }
        }
    }

    /// The libraries an asset can be saved into: the user's own first, then the further library folders.
    pub(super) fn libraries(&self) -> Vec<Library> {
        let user = self.tray.user_library().map(|p| p.to_path_buf());
        let mut out: Vec<Library> = user.iter().map(|p| Library { label: "My assets".into(), path: p.clone() }).collect();
        for p in self.tray.material_folders() {
            if Some(&p) != user.as_ref() {
                let label = p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned());
                out.push(Library { label, path: p });
            }
        }
        out
    }

    /// Completes `f` (its needed maps, the assets it uses) and writes it under `choice`'s name into its library.
    fn write_fragment(&mut self, mut f: Fragment, choice: &SmartAssetChoice, thumbnail: Option<Tex>) {
        f.name = choice.name.clone();
        // Assets the layers use come from the document's embedded copies, else from the library folders.
        let mut lookup = self.asset_lookup();
        let embedded = self.document.as_ref().map(|d| d.doc().assets.clone()).unwrap_or_default();
        let report = f.complete(&generators::baked_maps, |r| embedded.iter().find(|a| a.reference == r).map(|a| a.bytes.clone()).or_else(|| lookup(r)));
        let (usage, what) = match f.kind() {
            FragmentKind::SmartMaterial => (Usage::SmartMaterial, "smart material"),
            FragmentKind::SmartMask => (Usage::SmartMask, "smart mask"),
        };
        let options = SaveOptions { thumbnail: thumbnail.map_or(Thumbnail::FromDocument, Thumbnail::Image) };
        let root = LibraryRoot::folder("library", "library", RootKind::User, &choice.library);
        let mut bytes = Vec::new();
        let saved = fragment::save_with(&f, &mut bytes, &options).map_err(|e| e.to_string()).and_then(|()| {
            let change = AssetStore::new().create_named(&root, usage, &f.name, fragment::EXTENSION, &Draft::new(&bytes)).map_err(|e| e.to_string())?;
            Ok(change.after.map(|a| a.path).unwrap_or_default())
        });
        let path = choice.library.clone();
        match saved {
            Ok(rel) => {
                let path = path.join(rel);
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

    /// After `name` (an asset needing the baked maps `needs`, by id) was dropped onto texture set `set`: warns about
    /// the maps the set lacks and asks whether to bake them now (or does what the user told it to remember).
    pub(super) fn bake_missing_maps(&mut self, set: &str, name: &str, needs: &[String]) {
        let Some(doc) = &self.document else { return };
        let Some(missing) = MissingMaps::of(doc.doc(), set, needs) else { return };
        let warning = missing.warning(name);
        if missing.bakeable.is_empty() {
            self.diag.warn(Area::Bake, format!("{warning} Bake them in the Bakers panel."));
            return;
        }
        match self.shell.settings.get().prompts.missing_maps {
            MissingMapsAnswer::Ask => {
                self.diag.warn(Area::Bake, warning.clone());
                self.smart.bake = Some((BakePrompt::new(warning), set.to_string(), missing.bakeable));
            }
            MissingMapsAnswer::Bake => {
                self.diag.warn(Area::Bake, format!("{warning} Baking them now; the Bakers panel can cancel."));
                self.start_bake(&[set.to_string()], Some(MapSelection::of(&missing.bakeable)));
            }
            MissingMapsAnswer::Skip => self.diag.warn(Area::Bake, format!("{warning} Bake them in the Bakers panel.")),
        }
    }
}

fn what(f: &Fragment) -> &'static str {
    match f.kind() {
        FragmentKind::SmartMaterial => "smart material",
        FragmentKind::SmartMask => "smart mask",
    }
}

/// The thumbnail as an egui image (for the dialog's preview).
fn color_image(t: &Tex) -> egui::ColorImage {
    let px = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let pixels = t.px.iter().map(|p| egui::Color32::from_rgb(px(p[0]), px(p[1]), px(p[2]))).collect();
    egui::ColorImage { size: [t.w, t.h], source_size: egui::vec2(t.w as f32, t.h as f32), pixels }
}

/// "ambient occlusion, curvature".
fn map_names(maps: &[BakedKind]) -> String {
    maps.iter().map(|k| k.id().replace('_', " ")).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
#[path = "smart_assets_tests.rs"]
mod tests;
