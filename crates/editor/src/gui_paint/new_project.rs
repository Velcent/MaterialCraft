//! File > New project and "Save as project template" on the Paint screen. The dialog offers the built-in templates
//! and the `.mctemplate` files of the asset libraries; Create makes the document in the background through
//! `paint::new_project` ([`Loader::start_new`]) and opens it ready to paint and bake. A template is saved into a
//! library's `templates` folder through the asset store.

use super::PaintMode;
use super::dialogs::new_project::{NewProjectDialog, Outcome, TemplateEntry};
use super::dialogs::template_name::TemplateNameDialog;
use super::loader::Loader;
use eframe::egui;
use materialcraft::assets::{NewAsset, Provenance, Usage};
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::format::template;
use materialcraft::paint::new_project::ProjectTemplate;

/// The New project dialog and the template being named.
#[derive(Default)]
pub struct NewProjects {
    dialog: Option<NewProjectDialog>,
    naming: Option<(ProjectTemplate, TemplateNameDialog)>,
}

impl PaintMode {
    /// Opens the New project dialog (the open document was saved or given up already).
    pub(super) fn open_new_project(&mut self) {
        self.new_projects.dialog = Some(NewProjectDialog::new(self.project_templates()));
    }

    /// The built-in templates, then the libraries' readable template files (unreadable ones are logged).
    fn project_templates(&self) -> Vec<TemplateEntry> {
        let mut out: Vec<TemplateEntry> = ProjectTemplate::builtins().into_iter().map(|template| TemplateEntry { template, file: None }).collect();
        for (_, path) in self.tray.template_files() {
            let read = std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| template::from_text(&t).map_err(|e| e.to_string()));
            match read {
                Ok(template) => out.push(TemplateEntry { template, file: Some(path) }),
                Err(e) => self.diag.warn(Area::Assets, format!("Project template {} can't be read: {e}", path.display())),
            }
        }
        out
    }

    /// Shows the dialogs and acts on their answers.
    pub(super) fn poll_new_project(&mut self, ctx: &egui::Context) {
        if let Some(dialog) = &mut self.new_projects.dialog
            && let Some(outcome) = dialog.show(ctx)
        {
            self.new_projects.dialog = None;
            if let Outcome::Create(settings) = outcome {
                self.diag.info(Area::Project, format!("Making a new project from {}", settings.mesh.display()));
                self.loader = Some(Loader::start_new(*settings));
            }
        }
        if let Some((_, dialog)) = &mut self.new_projects.naming
            && let Some(answer) = dialog.show(ctx)
            && let Some((template, _)) = self.new_projects.naming.take()
            && let Some(choice) = answer
        {
            self.write_template(template, &choice.name, &choice.library);
        }
    }

    /// "Save as project template": the open document's defaults, named next.
    pub(super) fn save_template(&mut self) {
        let Some(doc) = &self.document else { return };
        let libraries = self.libraries();
        if libraries.is_empty() {
            self.diag.warn(Area::Assets, "There is no asset library to save the template into (settings are kept in memory).");
            return;
        }
        let name = doc.source().file_stem().map_or_else(|| "My template".to_string(), |s| s.to_string_lossy().into_owned());
        match ProjectTemplate::from_document(doc.doc(), &name) {
            Some(t) => self.new_projects.naming = Some((t, TemplateNameDialog::new(name, libraries))),
            None => self.diag.warn(Area::Assets, "A document without texture sets has no settings to save as a template."),
        }
    }

    fn write_template(&mut self, mut t: ProjectTemplate, name: &str, library: &std::path::Path) {
        t.name = name.to_string();
        let asset = NewAsset {
            usage: Usage::Template,
            name: name.to_string(),
            extension: template::EXTENSION.to_string(),
            bytes: template::to_text(&t).into_bytes(),
            thumbnail: None,
            tags: Vec::new(),
            description: None,
            provenance: Provenance::HandMade { author: None, notes: None },
        };
        match asset.save(library) {
            Ok(saved) => self.diag.info(Area::Assets, format!("Saved project template \"{name}\" to {}.", saved.path.display())),
            Err(e) => self.diag.warn(Area::Assets, format!("The project template \"{name}\" was not saved to {}: {e}.", library.display())),
        }
    }
}
