//! The library settings inside the tray (AS-18), usable without an open document: where the user's own library
//! is, the further library folders (add, remove), the layout and "hide empty folders".

use std::path::{Path, PathBuf};

use eframe::egui::{self, Ui};

use super::TrayEvent;
use crate::gui_settings::{TrayLayout, TraySettings};

#[derive(Default)]
pub struct SettingsState {
    /// A folder typed in by hand.
    typed: String,
}

pub struct Page<'a> {
    pub user: Option<&'a Path>,
    pub extra: &'a mut Vec<PathBuf>,
    pub view: &'a mut TraySettings,
    pub state: &'a mut SettingsState,
    pub events: &'a mut Vec<TrayEvent>,
}

impl Page<'_> {
    pub fn show(self, ui: &mut Ui) {
        let Page { user, extra, view, state, events } = self;
        egui::ScrollArea::vertical().id_salt("asset_tray_settings").show(ui, |ui| {
            ui.strong("Your library");
            match user {
                Some(p) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(p.display().to_string());
                        if ui.small_button("Copy").on_hover_text("Copy the folder path").clicked() {
                            ui.ctx().copy_text(p.display().to_string());
                        }
                    });
                    ui.weak("Put .mca materials and PNG or EXR images into its folders (materials, textures, …); the tray picks them up within a second.");
                }
                None => {
                    ui.weak("Settings are kept in memory only, so there is no library folder.");
                }
            }
            ui.add_space(8.0);
            ui.strong("Further libraries");
            let mut changed = false;
            let mut remove = None;
            for (i, p) in extra.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.small_button("🗑").on_hover_text("Remove this library (its files stay)").clicked() {
                        remove = Some(i);
                    }
                    let missing = !p.is_dir();
                    let text = p.display().to_string();
                    let r = if missing { ui.colored_label(egui::Color32::from_rgb(220, 120, 80), text) } else { ui.label(text) };
                    if missing {
                        r.on_hover_text("This folder does not exist (or can't be read).");
                    }
                });
            }
            if let Some(i) = remove.filter(|i| *i < extra.len()) {
                extra.remove(i);
                changed = true;
            }
            ui.horizontal(|ui| {
                if ui.button("Add folder…").on_hover_text("Choose a folder to use as a library").clicked()
                    && let Some(d) = rfd::FileDialog::new().pick_folder()
                {
                    changed |= add(extra, d);
                }
                let field = ui.add(egui::TextEdit::singleline(&mut state.typed).hint_text("or type a folder path").desired_width(180.0));
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if (ui.add_enabled(!state.typed.trim().is_empty(), egui::Button::new("Add")).clicked() || enter) && !state.typed.trim().is_empty() {
                    changed |= add(extra, PathBuf::from(state.typed.trim()));
                    state.typed.clear();
                }
            });
            if changed {
                events.push(TrayEvent::SetRoots(extra.clone()));
            }
            ui.add_space(8.0);
            ui.strong("Layout");
            let before = view.clone();
            ui.horizontal(|ui| {
                ui.radio_value(&mut view.layout, TrayLayout::Auto, "Automatic").on_hover_text("Wide when the panel is wide enough");
                ui.radio_value(&mut view.layout, TrayLayout::Wide, "Wide").on_hover_text("Type icons and the folder tree beside the assets");
                ui.radio_value(&mut view.layout, TrayLayout::Narrow, "Narrow").on_hover_text("Types in one menu, folders folded away");
            });
            ui.checkbox(&mut view.hide_empty, "Hide folders that match nothing")
                .on_hover_text("Leave out folders without assets of the chosen types and search");
            if *view != before {
                events.push(TrayEvent::SetView(view.clone()));
            }
        });
    }
}

/// Adds `dir` unless it is listed already.
fn add(extra: &mut Vec<PathBuf>, dir: PathBuf) -> bool {
    if extra.contains(&dir) {
        return false;
    }
    extra.push(dir);
    true
}
