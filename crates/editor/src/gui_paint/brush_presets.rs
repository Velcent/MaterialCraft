//! "Save brush as preset" (AS-06): the brush panel's button takes the shown brush tool's settings, paints a sample
//! stroke of them for the thumbnail, asks for the name and the library ([`BrushPresetDialog`]) and writes
//! `brushes/<name>.mcbrush` with its `.thumb.png` there, where the tray picks it up on its next poll.

use super::PaintMode;
use super::dialogs::brush_preset::{BrushPresetDialog, PresetChoice, PresetOutcome};
use eframe::egui;
use materialcraft::assets::{THUMBNAIL_SUFFIX, Usage, new_asset_path};
use materialcraft::deflate::Level;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::brush::{BrushPreset, file};
use materialcraft::png::{ColorType, encode_raw};
use materialcraft_brush::{AlphaSource, alpha_for, stroke_preview};
use std::path::{Path, PathBuf};

/// Side of a saved preset's thumbnail.
const THUMB_PX: u32 = 128;

/// The preset being named.
#[derive(Default)]
pub struct BrushPresets {
    naming: Option<Naming>,
}

enum Naming {
    /// Taken with its thumbnail; the dialog opens next frame.
    Made(BrushPreset, Vec<f32>),
    Asking(BrushPreset, Vec<f32>, BrushPresetDialog),
}

/// A sample stroke of `preset`, [`THUMB_PX`] square (its alpha-image tip from `alphas`).
pub fn thumbnail(preset: &BrushPreset, alphas: &dyn AlphaSource) -> Vec<f32> {
    stroke_preview(preset, alpha_for(&preset.tip, alphas), THUMB_PX, THUMB_PX)
}

/// Writes `preset` named `name` into `library`'s brushes folder with `thumbnail` ([`THUMB_PX`] square coverage) as
/// its picture; returns the preset's path. An existing preset is never replaced (the name gets a number).
pub fn write_preset(library: &Path, name: &str, preset: &BrushPreset, thumbnail: &[f32]) -> Result<PathBuf, String> {
    let preset = BrushPreset { name: name.to_string(), ..preset.clone() };
    let path = new_asset_path(library, Usage::Brush, name, file::EXTENSION);
    let dir = path.parent().ok_or("no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, preset.to_file_text()).map_err(|e| e.to_string())?;
    let samples: Vec<u8> = thumbnail.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).collect();
    if samples.len() == (THUMB_PX * THUMB_PX) as usize {
        let png = encode_raw(THUMB_PX as usize, THUMB_PX as usize, ColorType::Gray, 8, &samples, Level::Default);
        let mut thumb = path.clone().into_os_string();
        thumb.push(THUMBNAIL_SUFFIX);
        std::fs::write(PathBuf::from(thumb), png).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

impl PaintMode {
    /// The brush panel's "Save as preset…": the shown brush tool's settings, named next.
    pub(super) fn save_brush_preset(&mut self) {
        if self.libraries().is_empty() {
            self.diag.warn(Area::Assets, "There is no asset library to save the brush into (settings are kept in memory).");
            return;
        }
        let tool = self.painting.brush.brush_tool_for(self.shell.tools.current);
        let Some(preset) = self.painting.brush.preset(tool).cloned() else { return };
        let picture = thumbnail(&preset, &self.painting.alphas);
        self.brush_presets.naming = Some(Naming::Made(preset, picture));
    }

    /// Shows the naming dialog and saves once the user confirmed.
    pub(super) fn poll_brush_presets(&mut self, ctx: &egui::Context) {
        let (preset, picture, mut dialog) = match self.brush_presets.naming.take() {
            None => return,
            Some(Naming::Asking(p, picture, dialog)) => (p, picture, dialog),
            Some(Naming::Made(p, picture)) => {
                let preview = ctx.load_texture("brush_preset_thumbnail", grey_image(&picture), egui::TextureOptions::LINEAR);
                let dialog = BrushPresetDialog::new(p.name.clone(), self.libraries(), Some(preview));
                (p, picture, dialog)
            }
        };
        match dialog.show(ctx) {
            None => self.brush_presets.naming = Some(Naming::Asking(preset, picture, dialog)),
            Some(PresetOutcome::Cancel) => {}
            Some(PresetOutcome::Save(PresetChoice { name, library })) => match write_preset(&library, &name, &preset, &picture) {
                Ok(path) => self.diag.info(Area::Assets, format!("Saved brush preset \"{name}\" to {}.", path.display())),
                Err(e) => self.diag.warn(Area::Assets, format!("The brush preset \"{name}\" was not saved into {}: {e}.", library.display())),
            },
        }
    }
}

/// Coverage as an egui image (white paints).
fn grey_image(values: &[f32]) -> egui::ColorImage {
    let n = THUMB_PX as usize;
    let pixels = (0..n * n).map(|i| egui::Color32::from_gray((values.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
    egui::ColorImage { size: [n, n], source_size: egui::vec2(n as f32, n as f32), pixels }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::assets::{Library, LibraryRoot, RootId, RootKind, Thumbnail};
    use materialcraft::paint::brush::Tip;
    use materialcraft_brush::NoAlphas;

    /// The gate of roadmap step 45: a preset saved from the brush panel reloads bit for bit, and the tray lists it as
    /// a brush with its stroke thumbnail.
    #[test]
    fn a_saved_preset_reloads_bit_identically_and_shows_in_the_tray() {
        let dir = std::env::temp_dir().join(format!("mc-brush-presets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let preset = BrushPreset { tip: Tip::Procedural("spatter".into()), size: 0.137, spacing: 0.61, angle: 12.5, ..BrushPreset::default() };
        let picture = thumbnail(&preset, &NoAlphas);
        assert!(picture.iter().any(|v| *v > 0.5));
        let path = write_preset(&dir, "My spatter: 2", &preset, &picture).unwrap();
        assert_eq!(path, dir.join("brushes").join("My spatter_ 2.mcbrush"));
        let back = BrushPreset::from_file_text(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back, BrushPreset { name: "My spatter: 2".into(), ..preset.clone() });
        assert_eq!(back.size.to_bits(), preset.size.to_bits());
        let again = write_preset(&dir, "My spatter: 2", &preset, &picture).unwrap();
        assert_ne!(again, path, "an existing preset is never replaced");

        let lib = Library::with_roots([LibraryRoot::folder("user", "My assets", RootKind::User, &dir)], &materialcraft::diag::NullSink);
        let asset = lib.catalogue().get(&RootId::new("user"), "brushes/My spatter_ 2.mcbrush").unwrap().clone();
        assert_eq!(asset.usage, Usage::Brush);
        assert_eq!(asset.thumbnail, Thumbnail::File("brushes/My spatter_ 2.mcbrush.thumb.png".into()));
        assert!(dir.join("brushes/My spatter_ 2.mcbrush.thumb.png").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
