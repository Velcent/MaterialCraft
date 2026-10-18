//! File > New project: the mesh file, a template and the settings of [`NewProject`]. Settings a file type can't use
//! are greyed out (USD's scope, subdivision and frame; cameras for files without them), as are the features not built
//! yet (UV tiles, generating UVs). The dialog only collects the choices; `paint::new_project` makes the document.

mod sections;

use eframe::egui::{self, Id};
use materialcraft::mesh::LengthUnit;
use materialcraft::paint::new_project::{MeshFormat, NewProject, ProjectTemplate};
use std::path::PathBuf;

/// A template the user can pick, with where it came from.
pub struct TemplateEntry {
    pub template: ProjectTemplate,
    /// The library file; `None` for the built-in ones.
    pub file: Option<PathBuf>,
}

/// What the dialog returns once closed.
pub enum Outcome {
    Create(Box<NewProject>),
    Cancel,
}

pub struct NewProjectDialog {
    settings: NewProject,
    templates: Vec<TemplateEntry>,
    template: usize,
    /// The unit shown while "custom unit" is off, so switching back and forth keeps it.
    custom_unit: LengthUnit,
}

impl NewProjectDialog {
    /// A dialog offering `templates` (at least one; the first is picked).
    pub fn new(templates: Vec<TemplateEntry>) -> NewProjectDialog {
        let first = templates.first().map_or_else(|| ProjectTemplate::builtins().remove(0), |t| t.template.clone());
        NewProjectDialog { settings: NewProject::new(PathBuf::new(), first), templates, template: 0, custom_unit: LengthUnit::Centimetre }
    }

    /// The format of the chosen mesh file, if one is chosen and known.
    fn format(&self) -> Option<MeshFormat> {
        MeshFormat::of(&self.settings.mesh)
    }

    /// Why Create is not possible yet; `None` when it is.
    fn blocker(&self) -> Option<String> {
        if self.settings.mesh.as_os_str().is_empty() {
            return Some("Choose a mesh file.".into());
        }
        match self.format() {
            None => Some("This is not a mesh file MaterialCraft knows.".into()),
            Some(f) if !f.is_readable() => Some(format!("{} files can't be read yet: export the mesh as OBJ or glTF binary (.glb).", f.label())),
            Some(_) => None,
        }
    }

    /// Draws the dialog; `Some` once the user created or cancelled.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Outcome> {
        let mut outcome = None;
        let modal = egui::Modal::new(Id::new("paint_new_project_dialog")).show(ctx, |ui| {
            ui.set_width(520.0);
            ui.heading("New project");
            ui.label("Paint on a mesh from a file: every material of the mesh becomes its own texture set.");
            ui.add_space(6.0);
            let max_height = (ctx.content_rect().height() - 180.0).max(200.0);
            egui::ScrollArea::vertical().max_height(max_height).show(ui, |ui| {
                sections::basics(ui, self);
                sections::usd(ui, self);
                sections::normals(ui, self);
                sections::uv_tiles(ui);
                sections::imports(ui, self);
                sections::physical_size(ui, self);
                sections::colour(ui);
            });
            ui.add_space(8.0);
            let blocker = self.blocker();
            if let Some(b) = &blocker {
                ui.colored_label(ui.visuals().warn_fg_color, b);
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(blocker.is_none(), egui::Button::new("Create")).clicked() {
                    outcome = Some(Outcome::Create(Box::new(self.settings.clone())));
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
}
