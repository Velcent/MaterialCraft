//! Brush presets (AS-06): a handful of brushes built on the starter alphas, as `.mcbrush` files with a thumbnail of
//! a sample stroke. Brushes on PhotoCraft's tips take the procedural tip itself (it needs no image); the airbrush
//! takes the soft round alpha image by its library path, so the tray has an alpha-image brush too.

use crate::alphas::{ALPHA_PX, ROUND_SOFT, Round, alpha_path};
use crate::build::Options;
use crate::image::{THUMB_PX, grey_thumb};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::paint::brush::{BrushPreset, Jitter, PressureUse, Tip, file};
use materialcraft_brush::{TipAlpha, stroke_preview};

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    presets().into_iter().map(|(id, tags, preset)| Box::new(Preset { id, tags, preset }) as Box<dyn Recipe>).collect()
}

/// Every starter brush: its file id, its tags and the preset.
fn presets() -> Vec<(&'static str, &'static [&'static str], BrushPreset)> {
    let d = BrushPreset::default;
    let procedural = |id: &str| Tip::Procedural(id.into());
    let jitter = |size, angle, position| Jitter { size, flow: 0.0, angle, position };
    vec![
        ("basic-hard", &["round", "basic"], BrushPreset { name: "Basic hard".into(), hardness: 0.85, ..d() }),
        ("basic-soft", &["round", "basic", "soft"], BrushPreset { name: "Basic soft".into(), hardness: 0.0, flow: 0.5, ..d() }),
        (
            "airbrush",
            &["round", "soft", "alpha"],
            BrushPreset {
                name: "Airbrush".into(),
                tip: Tip::Alpha(alpha_path(ROUND_SOFT.0)),
                flow: 0.15,
                spacing: 0.05,
                pressure: PressureUse { size: false, flow: true },
                ..d()
            },
        ),
        ("chalk", &["chalk", "dry"], BrushPreset { name: "Chalk".into(), tip: procedural("chalk"), spacing: 0.15, jitter: jitter(0.1, 0.3, 0.0), ..d() }),
        ("charcoal", &["charcoal", "dry"], BrushPreset { name: "Charcoal".into(), tip: procedural("charcoal"), flow: 0.7, spacing: 0.12, ..d() }),
        (
            "dry-bristles",
            &["bristles", "dry"],
            BrushPreset { name: "Dry bristles".into(), tip: procedural("bristles"), follow_direction: true, spacing: 0.05, ..d() },
        ),
        (
            "spatter",
            &["spatter", "grunge"],
            BrushPreset { name: "Spatter".into(), tip: procedural("spatter"), spacing: 0.6, jitter: jitter(0.4, 1.0, 0.6), ..d() },
        ),
        (
            "sponge",
            &["sponge", "grunge"],
            BrushPreset { name: "Sponge dab".into(), tip: procedural("sponge"), spacing: 0.35, flow: 0.8, jitter: jitter(0.2, 1.0, 0.2), ..d() },
        ),
        (
            "rake-scratches",
            &["rake", "scratches"],
            BrushPreset { name: "Rake scratches".into(), tip: procedural("rake"), follow_direction: true, spacing: 0.04, ..d() },
        ),
        ("grass", &["grass", "nature"], BrushPreset { name: "Grass".into(), tip: procedural("grass"), spacing: 0.45, jitter: jitter(0.4, 0.15, 0.3), ..d() }),
        ("leaves", &["leaf", "nature"], BrushPreset { name: "Leaves".into(), tip: procedural("leaf"), spacing: 0.8, jitter: jitter(0.4, 1.0, 0.8), ..d() }),
        (
            "sparkles",
            &["sparkle", "stars"],
            BrushPreset { name: "Sparkles".into(), tip: procedural("sparkle"), spacing: 1.5, jitter: jitter(0.6, 0.5, 1.5), ..d() },
        ),
    ]
}

/// One starter brush preset.
struct Preset {
    id: &'static str,
    tags: &'static [&'static str],
    preset: BrushPreset,
}

impl Preset {
    /// The alpha image an alpha-tip preset paints with (the starter's own, made the same way).
    fn alpha(&self) -> Result<Option<TipAlpha>, StarterError> {
        match &self.preset.tip {
            Tip::Alpha(r) if *r == alpha_path(ROUND_SOFT.0) => {
                Ok(Some(TipAlpha::new(ALPHA_PX, ALPHA_PX, Round::soft().values()).ok_or_else(|| StarterError::recipe(&self.path(), "bad alpha size"))?))
            }
            Tip::Alpha(r) => Err(StarterError::recipe(&self.path(), &format!("no starter alpha {r}"))),
            Tip::Round | Tip::Procedural(_) => Ok(None),
        }
    }
}

impl Recipe for Preset {
    fn path(&self) -> String {
        format!("{}/{}.{}", Usage::Brush.folder(), self.id, file::EXTENSION)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let n = THUMB_PX;
        let stroke = stroke_preview(&self.preset, self.alpha()?, n as u32, n as u32);
        let at = |u: f32, v: f32| stroke.get((v * n as f32) as usize * n + (u * n as f32) as usize).copied().unwrap_or(0.0);
        let tip = match &self.preset.tip {
            Tip::Round => "a round tip".to_string(),
            Tip::Procedural(id) => format!("PhotoCraft's procedural {id} tip"),
            Tip::Alpha(r) => format!("the alpha {r}"),
        };
        let description = format!("{} brush preset with {tip}.", self.preset.name);
        let provenance = Provenance::Generator { generator: "mcstarter.brush_preset".into(), preset: Some(self.preset.to_json()) };
        Ok(Made {
            bytes: self.preset.to_file_text().into_bytes(),
            sidecar: sidecar(&self.preset.name, Usage::Brush, self.tags, &description, provenance),
            thumbnail: Some(grey_thumb(at)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_reads_back_and_names_a_known_tip() {
        let alphas: Vec<String> = crate::alphas::recipes().iter().map(|r| r.path()).collect();
        for (id, _, preset) in presets() {
            assert_eq!(BrushPreset::from_file_text(&preset.to_file_text()).unwrap(), preset, "{id}");
            match &preset.tip {
                Tip::Procedural(t) => assert!(materialcraft_brush::procedural_tip_alpha(t).is_some(), "{id}"),
                Tip::Alpha(p) => assert!(alphas.contains(p), "{id}: {p}"),
                Tip::Round => {}
            }
        }
    }
}
