//! "Save as smart material / smart mask": the asset's name, the library it goes into and the thumbnail it gets
//! (taken from the 3D view when the dialog opened).

use eframe::egui::{self, Id, TextureHandle};
use std::path::PathBuf;

/// What the user confirmed.
#[derive(Clone, PartialEq, Debug)]
pub struct SmartAssetChoice {
    pub name: String,
    /// The library root to save into (the asset goes into its usage folder there).
    pub library: PathBuf,
    /// Store the view's thumbnail (else the fragment's own: a smart material's base colour).
    pub use_thumbnail: bool,
}

/// One library the asset can go into.
pub struct Library {
    pub label: String,
    pub path: PathBuf,
}

pub struct SmartAssetDialog {
    /// "smart material" or "smart mask".
    what: &'static str,
    name: String,
    libraries: Vec<Library>,
    library: usize,
    thumbnail: Option<TextureHandle>,
    use_thumbnail: bool,
}

/// What the dialog returns once closed.
pub enum Outcome {
    Save(SmartAssetChoice),
    Cancel,
}

impl SmartAssetDialog {
    /// `name` is the suggested name, `libraries` the writable libraries (the user's own first).
    pub fn new(what: &'static str, name: String, libraries: Vec<Library>, thumbnail: Option<TextureHandle>) -> SmartAssetDialog {
        let use_thumbnail = thumbnail.is_some();
        SmartAssetDialog { what, name, libraries, library: 0, thumbnail, use_thumbnail }
    }

    /// Draws the dialog; `Some` once the user saved or cancelled.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Outcome> {
        let mut outcome = None;
        let modal = egui::Modal::new(Id::new("paint_smart_asset_dialog")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading(format!("Save as {}", self.what));
            egui::Grid::new("smart_asset_grid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Name");
                let edit = ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(260.0));
                if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && self.valid() {
                    outcome = Some(Outcome::Save(self.choice()));
                }
                ui.end_row();
                ui.label("Library");
                let current = self.libraries.get(self.library).map_or("", |l| l.label.as_str()).to_owned();
                egui::ComboBox::from_id_salt("smart_asset_library").selected_text(current).width(260.0).show_ui(ui, |ui| {
                    for (i, l) in self.libraries.iter().enumerate() {
                        ui.selectable_value(&mut self.library, i, &l.label).on_hover_text(l.path.display().to_string());
                    }
                });
                ui.end_row();
                ui.label("Thumbnail");
                ui.vertical(|ui| match &self.thumbnail {
                    Some(t) => {
                        ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(96.0, 96.0)));
                        ui.checkbox(&mut self.use_thumbnail, "Use the 3D view");
                    }
                    None => {
                        ui.weak("The 3D view could not be captured; the asset gets its own preview.");
                    }
                });
                ui.end_row();
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(self.valid(), egui::Button::new("Save")).clicked() {
                    outcome = Some(Outcome::Save(self.choice()));
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(Outcome::Cancel);
                }
            });
        });
        if outcome.is_none() && modal.should_close() {
            outcome = Some(Outcome::Cancel);
        }
        outcome
    }

    fn valid(&self) -> bool {
        !self.name.trim().is_empty() && self.libraries.get(self.library).is_some()
    }

    fn choice(&self) -> SmartAssetChoice {
        let library = self.libraries.get(self.library).map(|l| l.path.clone()).unwrap_or_default();
        SmartAssetChoice { name: self.name.trim().to_string(), library, use_thumbnail: self.use_thumbnail && self.thumbnail.is_some() }
    }
}
