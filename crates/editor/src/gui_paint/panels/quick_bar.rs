//! The brush tools' quick-access controls in the viewport toolbar (PT-14): size, flow, stroke opacity and spacing.
//! Lazy mouse, symmetry and the preview switches join them with their features.

use super::brush_panel::size_range;
use eframe::egui::{self, Ui};
use materialcraft::paint::brush::{BrushPreset, SizeSpace};

pub fn show(ui: &mut Ui, p: &mut BrushPreset) {
    let (range, _, _) = size_range(p.size_space);
    let size = match p.size_space {
        SizeSpace::Model => {
            let mut percent = p.size * 100.0;
            let r = ui.add(egui::DragValue::new(&mut percent).range(range.start() * 100.0..=range.end() * 100.0).speed(0.1).prefix("Size ").suffix(" %"));
            if r.changed() {
                p.size = percent / 100.0;
            }
            r
        }
        _ => ui.add(egui::DragValue::new(&mut p.size).range(range).speed(1.0).prefix("Size ").suffix(" px")),
    };
    size.on_hover_text("Brush size (on the model: a share of its radius)");
    percent(ui, "Flow", &mut p.flow, 0.0..=1.0).on_hover_text("Coverage each stamp adds");
    percent(ui, "Opacity", &mut p.opacity, 0.0..=1.0).on_hover_text("The most coverage the whole stroke reaches");
    percent(ui, "Spacing", &mut p.spacing, 0.01..=2.0).on_hover_text("Distance between stamps, as a share of the size");
}

fn percent(ui: &mut Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> egui::Response {
    let mut x = *v * 100.0;
    let r = ui.add(egui::DragValue::new(&mut x).range(range.start() * 100.0..=range.end() * 100.0).speed(0.5).prefix(format!("{label} ")).suffix(" %"));
    if r.changed() {
        *v = x / 100.0;
    }
    r
}
