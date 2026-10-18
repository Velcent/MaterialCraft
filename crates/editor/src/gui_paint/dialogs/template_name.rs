//! "Save as project template": the template's name and the library it goes into.

use super::smart_asset::Library;
use eframe::egui::{self, Id};
use std::path::PathBuf;

/// What the user confirmed.
#[derive(Clone, PartialEq, Debug)]
pub struct TemplateChoice {
    pub name: String,
    /// The library root to save into (the template goes into its `templates` folder).
    pub library: PathBuf,
}

pub struct TemplateNameDialog {
    name: String,
    libraries: Vec<Library>,
    library: usize,
}

impl TemplateNameDialog {
    /// `name` is the suggested name, `libraries` the writable libraries (the user's own first).
    pub fn new(name: String, libraries: Vec<Library>) -> TemplateNameDialog {
        TemplateNameDialog { name, libraries, library: 0 }
    }

    /// Draws the dialog; `Some(Some(choice))` once saved, `Some(None)` once cancelled.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Option<TemplateChoice>> {
        let mut outcome = None;
        let modal = egui::Modal::new(Id::new("paint_template_name_dialog")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Save as project template");
            ui.label("New projects made with it get this document's texture set channels, bake settings, shader, exposure and environment, and its project settings.");
            ui.add_space(6.0);
            egui::Grid::new("template_name_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                let edit = ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(260.0));
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && self.valid() {
                    outcome = Some(self.choice());
                }
                ui.end_row();
                ui.label("Library");
                let current = self.libraries.get(self.library).map_or("", |l| l.label.as_str()).to_owned();
                egui::ComboBox::from_id_salt("template_library").selected_text(current).width(260.0).show_ui(ui, |ui| {
                    for (i, l) in self.libraries.iter().enumerate() {
                        ui.selectable_value(&mut self.library, i, &l.label).on_hover_text(l.path.display().to_string());
                    }
                });
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.valid(), egui::Button::new("Save")).clicked() {
                    outcome = Some(self.choice());
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(None);
                }
            });
        });
        if outcome.is_none() && modal.should_close() {
            outcome = Some(None);
        }
        outcome
    }

    fn valid(&self) -> bool {
        !self.name.trim().is_empty() && self.libraries.get(self.library).is_some()
    }

    fn choice(&self) -> Option<TemplateChoice> {
        let library = self.libraries.get(self.library)?.path.clone();
        Some(TemplateChoice { name: self.name.trim().to_string(), library })
    }
}
