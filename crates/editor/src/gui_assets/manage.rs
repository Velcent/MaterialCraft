//! Managing assets from the tray (roadmap step 47): an asset's context menu offers rename, edit tags and description,
//! move to another library and delete; rename, edit and delete ask first in a small dialog. The tray only turns the
//! answers into [`StoreRequest`]s; the asset store does the work (on the library thread) and the log says how it
//! went. Assets of the read-only starter library are not editable.

use eframe::egui::{self, Id, Ui};
use materialcraft::assets::{MetaEdit, RootId, StoreRequest};

use super::results::Item;

/// A library an asset can be moved into: its id and display name.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveTarget {
    pub root: RootId,
    pub name: String,
}

/// What the user picked in an asset's context menu.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuChoice {
    Rename,
    Edit,
    Delete,
    MoveTo(RootId),
}

/// The context menu's entries for `item`; `targets` are the writable libraries.
pub fn menu(ui: &mut Ui, item: &Item, targets: &[MoveTarget]) -> Option<MenuChoice> {
    if !item.writable {
        ui.weak(format!("{} is read-only.", item.library));
        return None;
    }
    let mut choice = None;
    if ui.button("Rename…").clicked() {
        choice = Some(MenuChoice::Rename);
    }
    if ui.button("Edit tags and description…").clicked() {
        choice = Some(MenuChoice::Edit);
    }
    let others: Vec<&MoveTarget> = targets.iter().filter(|t| t.root != item.root).collect();
    ui.add_enabled_ui(!others.is_empty(), |ui| {
        ui.menu_button("Move to library", |ui| {
            for t in others {
                if ui.button(&t.name).clicked() {
                    choice = Some(MenuChoice::MoveTo(t.root.clone()));
                }
            }
        });
    });
    ui.separator();
    if ui.button("Delete…").clicked() {
        choice = Some(MenuChoice::Delete);
    }
    if choice.is_some() {
        ui.close();
    }
    choice
}

/// The question open about an asset.
enum Dialog {
    Rename { item: Item, name: String },
    Edit { item: Item, name: String, tags: String, description: String },
    Delete { item: Item },
}

/// The tray's asset-management state: at most one dialog at a time.
#[derive(Default)]
pub struct Manage {
    dialog: Option<Dialog>,
}

impl Manage {
    /// Acts on a menu choice: a move is a request at once, the others open their dialog.
    pub fn choose(&mut self, item: Item, choice: MenuChoice) -> Option<StoreRequest> {
        match choice {
            MenuChoice::MoveTo(to) => Some(StoreRequest::Move { root: item.root.clone(), folder: item_folder(&item).to_owned(), path: item.path, to }),
            MenuChoice::Rename => {
                self.dialog = Some(Dialog::Rename { name: item.name.clone(), item });
                None
            }
            MenuChoice::Edit => {
                let (name, tags, description) = (item.name.clone(), item.tags.join(", "), item.description.clone().unwrap_or_default());
                self.dialog = Some(Dialog::Edit { item, name, tags, description });
                None
            }
            MenuChoice::Delete => {
                self.dialog = Some(Dialog::Delete { item });
                None
            }
        }
    }

    /// Draws the open dialog; returns the request once the user confirmed.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<StoreRequest> {
        let dialog = self.dialog.as_mut()?;
        let mut answer = Answer::Open;
        let modal = egui::Modal::new(Id::new("asset_tray_manage")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            answer = match dialog {
                Dialog::Rename { item, name } => rename(ui, item, name),
                Dialog::Edit { item, name, tags, description } => edit(ui, item, name, tags, description),
                Dialog::Delete { item } => delete(ui, item),
            };
        });
        if modal.should_close() && matches!(answer, Answer::Open) {
            answer = Answer::Cancel;
        }
        match answer {
            Answer::Open => None,
            Answer::Cancel => {
                self.dialog = None;
                None
            }
            Answer::Confirm(request) => {
                self.dialog = None;
                Some(request)
            }
        }
    }
}

enum Answer {
    Open,
    Cancel,
    Confirm(StoreRequest),
}

/// The folder of `item` (the move keeps it, in the other library).
fn item_folder(item: &Item) -> &str {
    item.path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// OK and Cancel; OK enabled when `ok` is.
fn buttons(ui: &mut Ui, label: &str, ok: bool) -> (bool, bool) {
    ui.add_space(8.0);
    ui.horizontal(|ui| (ui.add_enabled(ok, egui::Button::new(label)).clicked(), ui.button("Cancel").clicked())).inner
}

fn rename(ui: &mut Ui, item: &Item, name: &mut String) -> Answer {
    ui.heading("Rename asset");
    ui.weak(&item.path);
    let edit = ui.add(egui::TextEdit::singleline(name).desired_width(320.0));
    let valid = !name.trim().is_empty();
    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    ui.weak("The file, its sidecar and its thumbnail are renamed together.");
    match buttons(ui, "Rename", valid) {
        _ if enter && valid => Answer::Confirm(StoreRequest::Rename { root: item.root.clone(), path: item.path.clone(), name: name.trim().to_owned() }),
        (true, _) => Answer::Confirm(StoreRequest::Rename { root: item.root.clone(), path: item.path.clone(), name: name.trim().to_owned() }),
        (_, true) => Answer::Cancel,
        _ => Answer::Open,
    }
}

fn edit(ui: &mut Ui, item: &Item, name: &mut String, tags: &mut String, description: &mut String) -> Answer {
    ui.heading("Edit asset");
    ui.weak(&item.path);
    egui::Grid::new("asset_tray_edit_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(name).desired_width(280.0));
        ui.end_row();
        ui.label("Tags");
        ui.add(egui::TextEdit::singleline(tags).desired_width(280.0).hint_text("comma, separated"));
        ui.end_row();
        ui.label("Description");
        ui.add(egui::TextEdit::multiline(description).desired_width(280.0).desired_rows(3));
        ui.end_row();
    });
    ui.weak("The name shown in the tray; the file keeps its name (Rename changes both).");
    match buttons(ui, "Save", true) {
        (true, _) => {
            let edit = MetaEdit {
                name: Some(name.trim().to_owned()),
                tags: Some(MetaEdit::tags_from_text(tags)),
                description: Some(Some(description.trim().to_owned()).filter(|d| !d.is_empty())),
            };
            Answer::Confirm(StoreRequest::Update { root: item.root.clone(), path: item.path.clone(), edit })
        }
        (_, true) => Answer::Cancel,
        _ => Answer::Open,
    }
}

fn delete(ui: &mut Ui, item: &Item) -> Answer {
    ui.heading("Delete asset?");
    ui.label(format!("\"{}\" ({}) is removed from {} with its sidecar and thumbnail.", item.name, item.path, item.library));
    ui.weak("Documents that use it keep their own copy. This can't be undone.");
    match buttons(ui, "Delete", true) {
        (true, _) => Answer::Confirm(StoreRequest::Delete { root: item.root.clone(), path: item.path.clone() }),
        (_, true) => Answer::Cancel,
        _ => Answer::Open,
    }
}
