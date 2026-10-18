//! "Save brush as preset" (AS-06): the preset's name and the library it goes into, with the sample stroke its
//! thumbnail will show.

use super::smart_asset::Library;
use eframe::egui::{self, Id, TextureHandle};
use std::path::PathBuf;

/// What the user confirmed.
#[derive(Clone, PartialEq, Debug)]
pub struct PresetChoice {
    pub name: String,
    /// The library root to save into (the preset goes into its `brushes` folder).
    pub library: PathBuf,
}

/// What the dialog returns once closed.
pub enum PresetOutcome {
    Save(PresetChoice),
    Cancel,
}

pub struct BrushPresetDialog {
    name: String,
    libraries: Vec<Library>,
    library: usize,
    preview: Option<TextureHandle>,
}

impl BrushPresetDialog {
    /// `name` is the suggested name, `libraries` the writable libraries (the user's own first).
    pub fn new(name: String, libraries: Vec<Library>, preview: Option<TextureHandle>) -> BrushPresetDialog {
        BrushPresetDialog { name, libraries, library: 0, preview }
    }

    fn valid(&self) -> bool {
        !self.name.trim().is_empty() && self.libraries.get(self.library).is_some()
    }

    fn choice(&self) -> Option<PresetChoice> {
        let library = self.libraries.get(self.library)?.path.clone();
        Some(PresetChoice { name: self.name.trim().to_string(), library })
    }

    /// Draws the dialog; `Some` once the user saved or cancelled.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<PresetOutcome> {
        let mut outcome = None;
        let modal = egui::Modal::new(Id::new("paint_brush_preset_dialog")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Save brush as preset");
            if let Some(t) = &self.preview {
                ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(96.0, 96.0)));
            }
            egui::Grid::new("brush_preset_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                let edit = ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(260.0));
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && self.valid() {
                    outcome = self.choice().map(PresetOutcome::Save);
                }
                ui.end_row();
                ui.label("Library");
                let current = self.libraries.get(self.library).map_or("", |l| l.label.as_str()).to_owned();
                egui::ComboBox::from_id_salt("brush_preset_library").selected_text(current).width(260.0).show_ui(ui, |ui| {
                    for (i, l) in self.libraries.iter().enumerate() {
                        ui.selectable_value(&mut self.library, i, &l.label).on_hover_text(l.path.display().to_string());
                    }
                });
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.valid(), egui::Button::new("Save")).clicked() {
                    outcome = self.choice().map(PresetOutcome::Save);
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(PresetOutcome::Cancel);
                }
            });
        });
        if modal.should_close() && outcome.is_none() {
            outcome = Some(PresetOutcome::Cancel);
        }
        outcome
    }
}
