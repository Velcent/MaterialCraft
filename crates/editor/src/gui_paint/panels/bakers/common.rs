//! The common settings of the selected set: output size, ray distances, sampling, dilation, high-poly source.

use super::BakerEvent;
use super::fields::{Fields, num};
use eframe::egui::{self, Ui};
use materialcraft::paint::bake_config::CommonBake;

const SIZES: [u32; 6] = [256, 512, 1024, 2048, 4096, 8192];

/// Draws the rows; a change becomes one [`BakerEvent::Common`] with the whole new block.
pub fn show(ui: &mut Ui, set: &str, c: &CommonBake, f: &mut Fields) {
    let emit = |f: &mut Fields, common: CommonBake, key| f.events.push(BakerEvent::Common { set: set.to_string(), common: Box::new(common), key });
    let sizes: Vec<(u32, String)> = SIZES.iter().map(|s| (*s, format!("{s} x {s}"))).collect();
    let options: Vec<(u32, &str)> = sizes.iter().map(|(s, l)| (*s, l.as_str())).collect();
    let mut size_options = options.clone();
    if !SIZES.contains(&c.size) {
        size_options.push((c.size, "(custom)"));
    }
    if let Some(size) = f.choice(ui, "common.size", "Output size", c.size, &size_options) {
        emit(f, CommonBake { size, ..c.clone() }, None);
    }
    if let Some((v, k)) = f.slider(ui, "common.max_dist", "Frontal distance", c.max_dist, num(0.0..=1.0, 0.001, 0.1)) {
        emit(f, CommonBake { max_dist: v, ..c.clone() }, Some(k));
    }
    let rear = c.rear_dist.unwrap_or(c.max_dist);
    if let Some((v, k)) = f.slider(ui, "common.rear_dist", "Rear distance", rear, num(0.0..=1.0, 0.001, c.max_dist)) {
        emit(f, CommonBake { rear_dist: Some(v), ..c.clone() }, Some(k));
    }
    if let Some((v, k)) = f.count(ui, "common.padding", "Dilation width", c.padding, 0..=256, 8) {
        emit(f, CommonBake { padding: v, ..c.clone() }, Some(k));
    }
    let samples = [(1, "None"), (2, "2 x 2"), (3, "3 x 3"), (4, "4 x 4")];
    if let Some(v) = f.choice(ui, "common.samples", "Antialiasing", c.samples.clamp(1, 4), &samples) {
        emit(f, CommonBake { samples: v, ..c.clone() }, None);
    }
    if let Some(v) = f.check(ui, "common.average_normals", "Average normals across hard edges", c.average_normals) {
        emit(f, CommonBake { average_normals: v, ..c.clone() }, None);
    }
    if let Some(v) = f.check(ui, "common.ignore_backfaces", "Ignore back faces", c.ignore_backfaces) {
        emit(f, CommonBake { ignore_backfaces: v, ..c.clone() }, None);
    }
    f.row(ui, "common.high", |ui, f| {
        let mut low = c.high.low_as_high;
        if ui.checkbox(&mut low, "Use the low-poly mesh as high-poly").changed() {
            let mut high = c.high.clone();
            high.low_as_high = low;
            emit(f, CommonBake { high, ..c.clone() }, None);
        }
        if !c.high.low_as_high {
            ui.horizontal(|ui| {
                ui.label("High-poly");
                let draft = egui::Id::new(("bakers_high_files", set));
                let saved = c.high.files.join("; ");
                let mut text = ui.data(|d| d.get_temp::<String>(draft)).unwrap_or_else(|| saved.clone());
                let resp = ui.add(egui::TextEdit::singleline(&mut text).hint_text("file.obj; …").desired_width(f32::INFINITY));
                resp.clone().on_hover_text("OBJ files: a path, or a file name looked up below the document's folder. Separate several with ;");
                if resp.has_focus() {
                    ui.data_mut(|d| d.insert_temp(draft, text.clone()));
                } else {
                    ui.data_mut(|d| d.remove::<String>(draft));
                }
                if resp.lost_focus() && text != saved {
                    let mut high = c.high.clone();
                    high.files = text.split(';').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
                    emit(f, CommonBake { high, ..c.clone() }, None);
                }
            });
        }
    });
}
