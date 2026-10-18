//! [`LibraryChoices`]: the import review's section for the user's own library (roadmap step 46). Per thing the project
//! brings (brush presets made from its strokes, bitmap alphas, stencils, textures, the environment map) an "Add to my
//! library" checkbox, unticked; items from Painter's shipped shelf say "from this project, not shareable". Ticked items
//! are written into the user's library after the review (the document embeds its own copies either way).

use eframe::egui::{self, RichText};
use materialcraft_substance::translate::library::{CandidateKind, LibraryCandidate, LibraryCandidates};
use std::sync::Arc;

pub struct LibraryChoices {
    candidates: Arc<LibraryCandidates>,
    /// Ticked per candidate, in the candidates' order.
    ticked: Vec<bool>,
    /// False when there is no user library to write into (the boxes are shown disabled).
    writable: bool,
}

impl LibraryChoices {
    /// Nothing is ticked, so the review adds nothing unless asked.
    pub fn new(candidates: Arc<LibraryCandidates>, writable: bool) -> LibraryChoices {
        let ticked = vec![false; candidates.items.len()];
        LibraryChoices { candidates, ticked, writable }
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    pub fn candidates(&self) -> &Arc<LibraryCandidates> {
        &self.candidates
    }

    /// The keys of the ticked candidates (empty when nothing can be written).
    pub fn ticked(&self) -> Vec<String> {
        if !self.writable {
            return Vec::new();
        }
        self.candidates.items.iter().zip(&self.ticked).filter(|(_, t)| **t).map(|(c, _)| c.key.clone()).collect()
    }

    /// Ticks or unticks every candidate of `kind`.
    pub fn set_kind(&mut self, kind: CandidateKind, on: bool) {
        for (c, t) in self.candidates.items.iter().zip(self.ticked.iter_mut()) {
            if c.kind == kind {
                *t = on;
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.strong("Add to my library");
        ui.small("Things this project brings, copied into your own asset library as external assets (your files; MaterialCraft ships none of them).");
        if !self.writable {
            ui.label(RichText::new("There is no library of your own to add them to: add one in the tray's library settings.").weak());
        }
        for kind in CandidateKind::ALL {
            let count = self.candidates.of_kind(kind).count();
            if count == 0 {
                continue;
            }
            let ticked = self.candidates.items.iter().zip(&self.ticked).filter(|(c, t)| c.kind == kind && **t).count();
            let title = format!("{} ({count}, {ticked} ticked)", heading(kind));
            egui::CollapsingHeader::new(title).id_salt(("import_review_library", kind.label())).default_open(count <= 8).show(ui, |ui| {
                ui.add_enabled_ui(self.writable, |ui| {
                    ui.horizontal(|ui| {
                        if ui.small_button("Tick all").clicked() {
                            self.set_kind(kind, true);
                        }
                        if ui.small_button("Untick all").clicked() {
                            self.set_kind(kind, false);
                        }
                    });
                    for (c, t) in self.candidates.items.iter().zip(self.ticked.iter_mut()) {
                        if c.kind == kind {
                            row(ui, c, t, &self.candidates);
                        }
                    }
                });
            });
        }
        if !self.candidates.left_out.is_empty() {
            ui.collapsing("Not offered", |ui| {
                for why in &self.candidates.left_out {
                    ui.small(why);
                }
            });
        }
    }
}

fn heading(kind: CandidateKind) -> &'static str {
    match kind {
        CandidateKind::Brush => "Brush presets from the strokes",
        CandidateKind::Alpha => "Alphas",
        CandidateKind::Stencil => "Stencils",
        CandidateKind::Texture => "Textures",
        CandidateKind::Environment => "Environment map",
    }
}

fn row(ui: &mut egui::Ui, c: &LibraryCandidate, ticked: &mut bool, all: &LibraryCandidates) {
    ui.horizontal(|ui| {
        let mut hover = format!("{}\nused by {} effect(s); from {}", c.name, c.uses, c.origin.describe());
        if let Some(alpha) = c.brings().and_then(|k| all.get(k)) {
            hover.push_str(&format!("\nbrings its alpha \"{}\" along", alpha.name));
        }
        ui.checkbox(ticked, &c.name).on_hover_text(hover);
        ui.small(format!("{} use(s)", c.uses));
        if !c.shareable() {
            ui.label(RichText::new("from this project, not shareable").weak().italics())
                .on_hover_text(format!("From {}: it stays in your own library and is never shared or shipped.", c.origin.describe()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_substance::translate::library::{CandidateSource, Origin};

    fn candidates() -> Arc<LibraryCandidates> {
        let item = |key: &str, kind, origin| LibraryCandidate {
            key: key.into(),
            kind,
            name: key.into(),
            origin,
            uses: 1,
            source: CandidateSource::Image { url: "/x?version=1".into() },
        };
        Arc::new(LibraryCandidates {
            project: "Cup.spp".into(),
            items: vec![
                item("alpha:a", CandidateKind::Alpha, Origin::Project),
                item("alpha:b", CandidateKind::Alpha, Origin::UserShelf),
                item("environment:e", CandidateKind::Environment, Origin::VendorShelf("viewer".into())),
            ],
            left_out: vec![],
        })
    }

    #[test]
    fn nothing_is_ticked_until_asked_and_without_a_library_nothing_is_written() {
        let mut c = LibraryChoices::new(candidates(), true);
        assert!(c.ticked().is_empty());
        c.set_kind(CandidateKind::Alpha, true);
        assert_eq!(c.ticked(), ["alpha:a", "alpha:b"]);
        c.ticked[2] = true;
        assert_eq!(c.ticked().len(), 3);
        c.set_kind(CandidateKind::Alpha, false);
        assert_eq!(c.ticked(), ["environment:e"]);
        let mut none = LibraryChoices::new(candidates(), false);
        none.set_kind(CandidateKind::Alpha, true);
        assert!(none.ticked().is_empty());
    }
}
