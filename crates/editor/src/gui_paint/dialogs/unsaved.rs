//! "Save changes?": asked before a document with unsaved changes is closed, replaced or the editor quits.

use eframe::egui::{self, Id};

/// What the user chose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Save first (Save As if the document has no file yet), then go on.
    Save,
    /// Go on and lose the changes.
    Discard,
    /// Do nothing.
    Cancel,
}

/// The question for one document; `doing` completes "before …" ("closing it", "quitting").
pub struct UnsavedPrompt {
    name: String,
    doing: &'static str,
}

impl UnsavedPrompt {
    pub fn new(name: String, doing: &'static str) -> UnsavedPrompt {
        UnsavedPrompt { name, doing }
    }

    /// Draws the dialog; `Some` once the user chose. Escape or a click beside it cancels.
    pub fn show(&self, ctx: &egui::Context) -> Option<Choice> {
        let mut choice = None;
        let modal = egui::Modal::new(Id::new("paint_unsaved_prompt")).show(ctx, |ui| {
            ui.set_max_width(380.0);
            ui.heading("Unsaved changes");
            ui.label(format!("Save the changes to \"{}\" before {}?", self.name, self.doing));
            ui.small("Without saving, the changes since the last save are lost.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    choice = Some(Choice::Save);
                }
                if ui.button("Don't save").clicked() {
                    choice = Some(Choice::Discard);
                }
                if ui.button("Cancel").clicked() {
                    choice = Some(Choice::Cancel);
                }
            });
        });
        if choice.is_none() && modal.should_close() {
            choice = Some(Choice::Cancel);
        }
        choice
    }
}
