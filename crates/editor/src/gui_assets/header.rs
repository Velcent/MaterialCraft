//! The top of the tray: saved-search tabs (AS-17), the live search field with the saved searches menu (AS-14,
//! AS-16), the usage type filter as icons (wide) or one menu (narrow) (AS-09, AS-12), the thumbnail size and the
//! button to the library settings. Every control has hover feedback (AS-20).

use eframe::egui::{self, Color32, Sense, Ui, vec2};
use materialcraft::assets::Usage;

use super::TrayEvent;
use super::icons;
use super::view::Filter;
use crate::gui_settings::{SavedSearch, ThumbSize, TraySettings};
use crate::gui_theme;

/// Help shown when hovering the search field.
const SEARCH_HELP: &str = "Search names, folders and tags. Words must all match (contains).\n\
    !word  exact match\n\"two words\"  a phrase (\\\" for a quote)\nsteel*  starts with   *steel  ends with\n\
    -word  leave out matches\nname:  tag:  path:  library:  usage:  format:  source:  limit a word to one field";

/// What the header keeps between frames.
#[derive(Default)]
pub struct HeaderState {
    pub settings_open: bool,
    /// The name typed for a new saved search.
    new_name: String,
}

/// One frame of the header.
pub struct Header<'a> {
    pub filter: &'a mut Filter,
    pub saved: &'a mut Vec<SavedSearch>,
    pub tab: &'a mut Option<usize>,
    pub view: &'a mut TraySettings,
    pub state: &'a mut HeaderState,
    pub events: &'a mut Vec<TrayEvent>,
}

impl Header<'_> {
    pub fn show(&mut self, ui: &mut Ui, wide: bool) {
        if self.saved.iter().any(|s| s.tab) {
            self.tabs(ui);
        }
        ui.horizontal(|ui| {
            let gear = ui.selectable_label(self.state.settings_open, "⚙").on_hover_text("Library settings: folders, layout");
            if gear.clicked() {
                self.state.settings_open = !self.state.settings_open;
            }
            self.saved_menu(ui);
            let field = egui::TextEdit::singleline(&mut self.filter.query).hint_text("Search assets").id_salt("asset_tray_search").desired_width(f32::INFINITY);
            ui.add(field).on_hover_text(SEARCH_HELP);
        });
        if self.state.settings_open {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if wide {
                self.type_icons(ui);
            } else {
                self.type_menu(ui);
            }
            ui.add_space(6.0);
            self.size_menu(ui);
        });
    }

    /// Tabs: "All" and every saved search kept as a tab; the active tab can be reset to its saved state.
    fn tabs(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(self.tab.is_none(), "All").on_hover_text("Every asset").clicked() {
                *self.tab = None;
                *self.filter = Filter::default();
            }
            let mut close = None;
            for (i, s) in self.saved.iter().enumerate().filter(|(_, s)| s.tab) {
                let active = *self.tab == Some(i);
                let saved = Filter::from_saved(s);
                let r = ui.selectable_label(active, &s.name).on_hover_text(saved.describe());
                if r.clicked() {
                    *self.tab = Some(i);
                    *self.filter = saved.clone();
                }
                if active && *self.filter != saved && ui.small_button("⟲").on_hover_text("Reset the tab to its saved search").clicked() {
                    *self.filter = saved;
                }
                if ui.small_button("×").on_hover_text("Close the tab (the saved search stays)").clicked() {
                    close = Some(i);
                }
            }
            if let Some(i) = close {
                if let Some(s) = self.saved.get_mut(i) {
                    s.tab = false;
                }
                if *self.tab == Some(i) {
                    *self.tab = None;
                }
                self.events.push(TrayEvent::SetSavedSearches(self.saved.clone()));
            }
        });
    }

    /// The saved searches: run one, keep it as a tab, delete it, or save the current filter.
    fn saved_menu(&mut self, ui: &mut Ui) {
        let mut changed = false;
        ui.menu_button("★", |ui| {
            if self.saved.is_empty() {
                ui.weak("No saved searches yet.");
            }
            let mut remove = None;
            for (i, s) in self.saved.iter_mut().enumerate() {
                let f = Filter::from_saved(s);
                ui.horizontal(|ui| {
                    if ui.button(&s.name).on_hover_text(f.describe()).clicked() {
                        *self.filter = f;
                        *self.tab = s.tab.then_some(i);
                        ui.close();
                    }
                    let tab = if s.tab { "Untab" } else { "Tab" };
                    if ui.small_button(tab).on_hover_text("Show this search as its own tab").clicked() {
                        s.tab = !s.tab;
                        changed = true;
                    }
                    if ui.small_button("🗑").on_hover_text("Delete this saved search").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove.filter(|i| *i < self.saved.len()) {
                self.saved.remove(i);
                *self.tab = None;
                changed = true;
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.state.new_name).hint_text("Name").desired_width(110.0));
                let name = self.state.new_name.trim().to_owned();
                if ui.add_enabled(!name.is_empty(), egui::Button::new("Save search")).on_hover_text("Keep the type filter, folder and query").clicked() {
                    self.saved.retain(|s| s.name != name);
                    self.saved.push(self.filter.to_saved(&name, false));
                    self.state.new_name.clear();
                    changed = true;
                }
            });
        })
        .response
        .on_hover_text("Saved searches");
        if changed {
            self.events.push(TrayEvent::SetSavedSearches(self.saved.clone()));
        }
    }

    /// One toggle icon per usage (wide layout).
    fn type_icons(&mut self, ui: &mut Ui) {
        for usage in Usage::ALL {
            let on = self.filter.shows(usage);
            let (rect, r) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
            let bg = if on {
                gui_theme::STRIP_SELECTED
            } else if r.hovered() {
                gui_theme::STRIP_HOVER
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 3.0, bg);
            let color = if on { gui_theme::ACCENT } else { gui_theme::ICON };
            icons::paint(ui.painter(), rect.shrink(4.0), usage, color);
            if r.on_hover_text(format!("{} (click to show or hide; none selected shows all)", icons::label(usage))).clicked() {
                self.filter.toggle(usage);
            }
        }
        if !self.filter.usages.is_empty() && ui.small_button("All").on_hover_text("Show every type").clicked() {
            self.filter.usages.clear();
        }
    }

    /// The usages in one menu (narrow layout).
    fn type_menu(&mut self, ui: &mut Ui) {
        let title = match self.filter.usages.as_slice() {
            [] => "All types".to_owned(),
            [one] => icons::label(*one).to_owned(),
            many => format!("{} types", many.len()),
        };
        ui.menu_button(title, |ui| {
            for usage in Usage::ALL {
                let mut on = self.filter.shows(usage);
                if ui.checkbox(&mut on, icons::label(usage)).changed() {
                    self.filter.toggle(usage);
                }
            }
            if ui.button("All types").clicked() {
                self.filter.usages.clear();
            }
        })
        .response
        .on_hover_text("Asset types to show");
    }

    fn size_menu(&mut self, ui: &mut Ui) {
        let sizes = [(ThumbSize::Small, "Small"), (ThumbSize::Medium, "Medium"), (ThumbSize::Large, "Large"), (ThumbSize::List, "List")];
        let current = sizes.iter().find(|(s, _)| *s == self.view.thumbnails).map_or("Medium", |(_, n)| n);
        let mut chosen = None;
        ui.menu_button(format!("▦ {current}"), |ui| {
            for (s, name) in sizes {
                if ui.radio(self.view.thumbnails == s, name).clicked() {
                    chosen = Some(s);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Thumbnail size, or a list");
        if let Some(s) = chosen {
            self.view.thumbnails = s;
            self.events.push(TrayEvent::SetView(self.view.clone()));
        }
    }
}
