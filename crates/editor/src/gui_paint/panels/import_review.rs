//! [`ReviewDialog`]: the import review. Per Substance procedural the project uses: MaterialCraft's replacement
//! (editable, with its measured accuracy against Painter, median MAE in 8-bit steps) or Painter's own result
//! (exact look, flattened, not editable). Procedurals without a replacement are listed as kept.

use eframe::egui::{self, RichText};
use materialcraft_substance::tools::analyze::{Analysis, ProceduralSummary};
use materialcraft_substance::translate::replacements::{Choice, ProceduralPlan};
use std::path::Path;

/// What the user decided.
pub enum ReviewResult {
    Import(ProceduralPlan),
    Cancel,
}

pub struct ReviewDialog {
    analysis: Analysis,
    /// The choice per replaceable procedural, by name.
    choices: Vec<(String, Choice)>,
}

impl ReviewDialog {
    /// Every replaceable procedural starts as Painter's result, so an import changes nothing unless asked.
    pub fn new(analysis: Analysis) -> ReviewDialog {
        let choices = analysis.procedurals.iter().filter(|p| p.replacement.is_some()).map(|p| (p.name.clone(), Choice::Keep)).collect();
        ReviewDialog { analysis, choices }
    }

    /// The plan the current choices make.
    pub fn plan(&self) -> ProceduralPlan {
        self.choices.iter().fold(ProceduralPlan::default(), |plan, (name, c)| plan.with(name, *c))
    }

    fn set_all(&mut self, choice: Choice) {
        self.choices.iter_mut().for_each(|(_, c)| *c = choice);
    }

    /// Draws the dialog; `Some` once the user imports or cancels.
    pub fn show(&mut self, ctx: &egui::Context, source: &Path) -> Option<ReviewResult> {
        let mut result = None;
        let name = source.file_name().map_or_else(|| source.display().to_string(), |n| n.to_string_lossy().into_owned());
        egui::Window::new("Import review").collapsible(false).resizable(true).default_width(620.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(
            ctx,
            |ui| {
                ui.label(format!("{name} uses Substance procedurals. Choose per procedural how it is imported:"));
                ui.small("MaterialCraft's replacement: editable layers, close to Painter (accuracy measured on your projects).");
                ui.small("Painter's result: exactly Painter's look, flattened, not editable.");
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Replace all").clicked() {
                        self.set_all(Choice::Replace);
                    }
                    if ui.button("Painter's result for all").clicked() {
                        self.set_all(Choice::Keep);
                    }
                });
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    egui::Grid::new("import_review_grid").striped(true).num_columns(4).show(ui, |ui| {
                        ui.strong("Procedural");
                        ui.strong("Uses");
                        ui.strong("Accuracy");
                        ui.strong("Import as");
                        ui.end_row();
                        for p in self.analysis.procedurals.iter().filter(|p| p.replacement.is_some()) {
                            let Some((_, choice)) = self.choices.iter_mut().find(|(n, _)| *n == p.name) else { continue };
                            row(ui, p, choice);
                        }
                    });
                    let kept: Vec<&ProceduralSummary> = self.analysis.procedurals.iter().filter(|p| p.replacement.is_none()).collect();
                    if !kept.is_empty() {
                        ui.add_space(6.0);
                        ui.collapsing(format!("{} more without a MaterialCraft replacement yet (Painter's result)", kept.len()), |ui| {
                            for p in kept {
                                ui.small(format!("{} — {} uses", p.name, p.uses.len()));
                            }
                        });
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Import").clicked() {
                        result = Some(ReviewResult::Import(self.plan()));
                    }
                    if ui.button("Cancel").clicked() {
                        result = Some(ReviewResult::Cancel);
                    }
                });
            },
        );
        result
    }
}

fn row(ui: &mut egui::Ui, p: &ProceduralSummary, choice: &mut Choice) {
    let label = p.replacement.map_or("", |r| r.label);
    ui.label(&p.name).on_hover_text(p.replacement.map_or(String::new(), |r| format!("{label}\n{}", r.note)));
    ui.label(format!("{} ({} shown)", p.uses.len(), p.visible_uses()));
    match &p.accuracy {
        Some(a) => ui.label(format!("{:.1}/255", a.median_mae)).on_hover_text(format!("median MAE over {} measured cases (mcparity run {})", a.cases, a.run)),
        None => ui.label(RichText::new("not measured").weak()),
    };
    ui.horizontal(|ui| {
        ui.radio_value(choice, Choice::Replace, "MaterialCraft");
        ui.radio_value(choice, Choice::Keep, "Painter's result");
    });
    ui.end_row();
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft_substance::tools::accuracy::ReplacementAccuracy;
    use materialcraft_substance::tools::analyze::{ProceduralUse, Role};

    fn use_of(name: &str) -> ProceduralUse {
        ProceduralUse { name: name.into(), role: Role::MaskFill, set: "S".into(), layer: "L".into(), visible: true, params: vec![], inputs: vec![], material: false }
    }

    #[test]
    fn choices_start_as_painters_result_and_make_the_plan() {
        let analysis = Analysis::of_uses("p.spp", vec![use_of("mg_dirt"), use_of("PBR_to_Skyrim"), use_of("mg_dirt")], &ReplacementAccuracy::default());
        let mut d = ReviewDialog::new(analysis);
        assert_eq!(d.choices, vec![("mg_dirt".to_string(), Choice::Keep)]);
        assert_eq!(d.plan().choice("mg_dirt"), Choice::Keep);
        d.set_all(Choice::Replace);
        let plan = d.plan();
        assert_eq!((plan.choice("mg_dirt"), plan.choice("PBR_to_Skyrim")), (Choice::Replace, Choice::Keep));
    }
}
