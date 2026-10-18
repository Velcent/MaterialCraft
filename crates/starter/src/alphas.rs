//! Brush alphas: PhotoCraft's procedural tips (through the brush crate, ADR 0008 rm-starter-scope) and two round
//! tips of our own, as 16-bit grey PNGs (white paints).

use crate::build::Options;
use crate::image::{THUMB_PX, grey_thumb, grey16};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::json::Value;
use materialcraft_brush::{PROCEDURAL_TIP_GENERATOR, PROCEDURAL_TIPS, TipInfo, procedural_tip};

/// Side of every alpha, in pixels.
pub const ALPHA_PX: u32 = 256;

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    let mut all: Vec<Box<dyn Recipe>> = vec![Box::new(Round { id: "round-hard", name: "Round hard", hardness: 0.95 }), Box::new(Round::soft())];
    all.extend(PROCEDURAL_TIPS.iter().map(|t| Box::new(PhotoCraftTip(t)) as Box<dyn Recipe>));
    all
}

/// The library path of the alpha `id` (`alphas/<id>.png`).
pub(crate) fn alpha_path(id: &str) -> String {
    format!("{}/{id}.png", Usage::Alpha.folder())
}

/// The id, name and hardness of the soft round alpha (the brush presets use it).
pub(crate) const ROUND_SOFT: (&str, &str, f32) = ("round-soft", "Round soft", 0.0);

/// A round tip: full inside `hardness` of the radius, a smooth falloff to the rim.
pub(crate) struct Round {
    id: &'static str,
    name: &'static str,
    hardness: f32,
}

impl Round {
    pub(crate) fn soft() -> Round {
        Round { id: ROUND_SOFT.0, name: ROUND_SOFT.1, hardness: ROUND_SOFT.2 }
    }

    fn value(&self, u: f32, v: f32) -> f32 {
        let r = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt() * 2.0;
        let t = ((1.0 - r) / (1.0 - self.hardness).max(1e-3)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// The tip, [`ALPHA_PX`] square, row by row.
    pub(crate) fn values(&self) -> Vec<f32> {
        let n = ALPHA_PX as usize;
        (0..n * n).map(|i| self.value(((i % n) as f32 + 0.5) / n as f32, ((i / n) as f32 + 0.5) / n as f32)).collect()
    }
}

impl Recipe for Round {
    fn path(&self) -> String {
        alpha_path(self.id)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let n = ALPHA_PX as usize;
        let values = self.values();
        let preset = Value::object().with("shape", "round").with("hardness", self.hardness).with("size", n);
        let description = format!("Round brush tip, hardness {:.2}, {n} × {n} pixels, 16-bit.", self.hardness);
        Ok(Made {
            bytes: grey16(n, n, &values),
            sidecar: sidecar(
                self.name,
                Usage::Alpha,
                &["round", "brush"],
                &description,
                Provenance::Generator { generator: "mcstarter.round_tip".into(), preset: Some(preset) },
            ),
            thumbnail: Some(grey_thumb(|u, v| self.value(u, v))),
        })
    }
}

/// One of PhotoCraft's procedural tips.
struct PhotoCraftTip(&'static TipInfo);

impl Recipe for PhotoCraftTip {
    fn path(&self) -> String {
        alpha_path(self.0.id)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let tip = |side: u32| procedural_tip(self.0.id, side).ok_or_else(|| StarterError::recipe(&self.path(), "PhotoCraft has no such tip"));
        let n = ALPHA_PX as usize;
        let values = tip(ALPHA_PX)?;
        let small = tip(THUMB_PX as u32)?;
        let at = |u: f32, v: f32| {
            let (x, y) = ((u * THUMB_PX as f32) as usize, (v * THUMB_PX as f32) as usize);
            small.get(y * THUMB_PX + x).copied().unwrap_or(0.0)
        };
        // `tip` names the procedural tip, so a brush can take the tip itself instead of this image
        let preset = Value::object().with("function", self.0.recipe).with("size", n).with("tip", self.0.id);
        let description = format!("{} brush tip from PhotoCraft's procedural tips, {n} × {n} pixels, 16-bit.", self.0.label);
        let provenance = Provenance::Generator { generator: PROCEDURAL_TIP_GENERATOR.into(), preset: Some(preset) };
        Ok(Made {
            bytes: grey16(n, n, &values),
            sidecar: sidecar(self.0.label, Usage::Alpha, &["brush", "photocraft", self.0.id], &description, provenance),
            thumbnail: Some(grey_thumb(at)),
        })
    }
}
