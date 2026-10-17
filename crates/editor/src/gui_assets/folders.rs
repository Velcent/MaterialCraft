//! Where the tray looks: the breadcrumb path of the chosen folder (AS-11) and the folder tree of every library
//! (AS-13). A folder holding nothing that matches the type filter and query is greyed out, or left out with
//! "hide empty"; picking a folder narrows the search to it and everything below it.

use std::collections::HashSet;

use eframe::egui::{self, RichText, Ui};
use materialcraft::assets::{Location, RootId};

use super::results::Results;
use super::view::Filter;
use crate::gui_theme;

/// Indent per folder level, in points.
const INDENT: f32 = 12.0;

/// Which folders are unfolded.
#[derive(Default)]
pub struct FolderState {
    open: HashSet<(RootId, String)>,
    /// Libraries folded shut (they start open).
    closed_roots: HashSet<RootId>,
}

/// "All libraries › My assets › materials › metal", each part clickable.
pub fn breadcrumbs(ui: &mut Ui, results: &Results, filter: &mut Filter) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut go = None;
        if ui.link("All libraries").on_hover_text("Search every library").clicked() {
            go = Some(None);
        }
        if let Some(loc) = &filter.location {
            let name = results.roots.iter().find(|r| r.id == loc.root).map_or(loc.root.as_str(), |r| r.name.as_str());
            ui.weak("›");
            if ui.link(name).on_hover_text("The whole library").clicked() {
                go = Some(Some(Location::root(loc.root.clone())));
            }
            let mut path = String::new();
            for part in loc.folder.split('/').filter(|p| !p.is_empty()) {
                if !path.is_empty() {
                    path.push('/');
                }
                path.push_str(part);
                ui.weak("›");
                let empty = results.count(&loc.root, &path) == 0;
                let text = if empty { RichText::new(part).color(gui_theme::ICON_DISABLED) } else { RichText::new(part) };
                if ui.link(text).on_hover_text(format!("{path} ({} matching)", results.count(&loc.root, &path))).clicked() {
                    go = Some(Some(Location::folder(loc.root.clone(), path.clone())));
                }
            }
        }
        ui.weak(format!("   {} assets", results.items.len()));
        if let Some(to) = go {
            filter.location = to;
        }
    });
}

/// The folder tree of every library.
pub fn tree(ui: &mut Ui, results: &Results, filter: &mut Filter, state: &mut FolderState, hide_empty: bool) {
    egui::ScrollArea::vertical().id_salt("asset_tray_tree").auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        for root in &results.roots {
            let open = !state.closed_roots.contains(&root.id);
            let count = results.count(&root.id, "");
            ui.horizontal(|ui| {
                if ui.small_button(if open { "▾" } else { "▸" }).on_hover_text("Fold or unfold").clicked() && !state.closed_roots.remove(&root.id) {
                    state.closed_roots.insert(root.id.clone());
                }
                let chosen = filter.location.as_ref().is_some_and(|l| l.root == root.id && l.folder.is_empty());
                if ui.selectable_label(chosen, RichText::new(&root.name).strong()).on_hover_text(format!("{count} matching assets")).clicked() {
                    filter.location = Some(Location::root(root.id.clone()));
                }
            });
            if !open {
                continue;
            }
            for (i, folder) in root.folders.iter().enumerate() {
                let n = results.count(&root.id, folder);
                if (hide_empty && n == 0) || !ancestors_open(state, &root.id, folder) {
                    continue;
                }
                let depth = folder.matches('/').count() + 1;
                let prefix = format!("{folder}/");
                let has_children = root.folders.get(i + 1).is_some_and(|next| next.starts_with(&prefix));
                let key = (root.id.clone(), folder.clone());
                ui.horizontal(|ui| {
                    ui.add_space(INDENT * depth as f32);
                    if has_children {
                        let unfolded = state.open.contains(&key);
                        if ui.small_button(if unfolded { "▾" } else { "▸" }).clicked() && !state.open.remove(&key) {
                            state.open.insert(key.clone());
                        }
                    } else {
                        ui.add_space(16.0);
                    }
                    let name = folder.rsplit('/').next().unwrap_or(folder);
                    let text = if n == 0 { RichText::new(name).color(gui_theme::ICON_DISABLED) } else { RichText::new(name) };
                    let chosen = filter.location.as_ref().is_some_and(|l| l.root == root.id && l.folder == *folder);
                    let hover = if n == 0 { "Nothing here matches the type filter and search".to_owned() } else { format!("{n} matching assets") };
                    if ui.selectable_label(chosen, text).on_hover_text(hover).clicked() {
                        filter.location = Some(Location::folder(root.id.clone(), folder.clone()));
                    }
                });
            }
        }
        if results.roots.is_empty() {
            ui.weak("No libraries. Add a folder in the library settings (⚙).");
        }
    });
}

/// True if every folder above `folder` is unfolded.
fn ancestors_open(state: &FolderState, root: &RootId, folder: &str) -> bool {
    let mut f = folder;
    while let Some((parent, _)) = f.rsplit_once('/') {
        if !state.open.contains(&(root.clone(), parent.to_owned())) {
            return false;
        }
        f = parent;
    }
    true
}
