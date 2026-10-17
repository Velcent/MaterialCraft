//! "Bake the missing maps?": asked after a smart material or mask that needs baked maps the texture set lacks was
//! dropped, instead of starting a bake right away. "Don't ask again" remembers the answer in the settings.

use eframe::egui::{self, Id};

/// What the user chose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BakeAnswer {
    pub bake: bool,
    /// Remember the answer and stop asking.
    pub remember: bool,
}

pub struct BakePrompt {
    /// The question (which asset needs which maps on which set).
    text: String,
    remember: bool,
}

impl BakePrompt {
    pub fn new(text: String) -> BakePrompt {
        BakePrompt { text, remember: false }
    }

    /// Draws the dialog; `Some` once the user chose. Escape means "Not now".
    pub fn show(&mut self, ctx: &egui::Context) -> Option<BakeAnswer> {
        let mut bake = None;
        let modal = egui::Modal::new(Id::new("paint_bake_missing_prompt")).show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.heading("Bake the missing maps?");
            ui.label(&self.text);
            ui.small("Until they are baked the effects that read them see nothing. A bake can be cancelled in the Bakers panel.");
            ui.add_space(6.0);
            ui.checkbox(&mut self.remember, "Don't ask again");
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Bake now").clicked() {
                    bake = Some(true);
                }
                if ui.button("Not now").clicked() {
                    bake = Some(false);
                }
            });
        });
        if bake.is_none() && modal.should_close() {
            bake = Some(false);
        }
        bake.map(|bake| BakeAnswer { bake, remember: self.remember })
    }
}
