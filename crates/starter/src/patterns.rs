//! Pattern textures: PhotoCraft's procedural patterns (paper tooth, woven canvas, value noise, halftone dots; through
//! the brush crate) as seamless 16-bit grey PNGs, for brush grain and fine surface detail. The sidecar records the
//! style, seed and size, so each one can be made again.

use crate::build::Options;
use crate::image::{THUMB_PX, grey_thumb, grey16};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::json::Value;
use materialcraft_brush::{PATTERN_GENERATOR, PATTERN_PERIOD, PatternKind, pattern};

/// Side of every pattern texture, in pixels: a multiple of [`PATTERN_PERIOD`], so the periodic styles tile.
pub const PATTERN_SIDE: u32 = 512;

/// One pattern texture: file id, display name, style, seed and tags.
struct PatternRecipe {
    id: &'static str,
    name: &'static str,
    kind: PatternKind,
    seed: u32,
    tags: &'static [&'static str],
}

const fn p(id: &'static str, name: &'static str, kind: PatternKind, seed: u32, tags: &'static [&'static str]) -> PatternRecipe {
    PatternRecipe { id, name, kind, seed, tags }
}

const PATTERNS: &[PatternRecipe] = &[
    p("paper-tooth", "Paper tooth", PatternKind::Paper, 1, &["pattern", "paper", "grain"]),
    p("paper-tooth-2", "Paper tooth 2", PatternKind::Paper, 2, &["pattern", "paper", "grain"]),
    p("paper-tooth-3", "Paper tooth 3", PatternKind::Paper, 3, &["pattern", "paper", "grain"]),
    p("canvas-weave", "Canvas weave", PatternKind::Canvas, 1, &["pattern", "canvas", "fabric", "grain"]),
    p("canvas-weave-2", "Canvas weave 2", PatternKind::Canvas, 2, &["pattern", "canvas", "fabric", "grain"]),
    p("value-noise", "Value noise", PatternKind::Noise, 1, &["pattern", "noise", "soft"]),
    p("value-noise-2", "Value noise 2", PatternKind::Noise, 2, &["pattern", "noise", "soft"]),
    // the dots take no seed
    p("halftone-dots", "Halftone dots", PatternKind::Dots, 1, &["pattern", "dots", "halftone"]),
];

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    PATTERNS.iter().map(|p| Box::new(Pattern(p)) as Box<dyn Recipe>).collect()
}

struct Pattern(&'static PatternRecipe);

impl Recipe for Pattern {
    fn path(&self) -> String {
        format!("{}/{}.png", Usage::Texture.folder(), self.0.id)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let side = PATTERN_SIDE;
        let values = pattern(self.0.kind, side, self.0.seed).ok_or_else(|| StarterError::recipe(&self.path(), "PhotoCraft made no pattern"))?;
        let n = side as usize;
        // the thumbnail shows a 128-pixel corner at full resolution, so the grain stays visible
        let at = |u: f32, v: f32| {
            let (x, y) = ((u * THUMB_PX as f32) as usize, (v * THUMB_PX as f32) as usize);
            values.get(y * n + x).copied().unwrap_or(0.0)
        };
        let preset = Value::object().with("style", self.0.kind.id()).with("seed", self.0.seed).with("size", n).with("period", PATTERN_PERIOD);
        let description = format!("Seamless grey pattern from PhotoCraft's procedural patterns ({} style), {n} × {n} pixels, 16-bit.", self.0.kind.id());
        let mut tags = self.0.tags.to_vec();
        tags.push("photocraft");
        Ok(Made {
            bytes: grey16(n, n, &values),
            sidecar: sidecar(
                self.0.name,
                Usage::Texture,
                &tags,
                &description,
                Provenance::Generator { generator: PATTERN_GENERATOR.into(), preset: Some(preset) },
            ),
            thumbnail: Some(grey_thumb(at)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_the_side_tiles() {
        for p in PATTERNS {
            assert_eq!(PATTERNS.iter().filter(|o| o.id == p.id).count(), 1, "{}", p.id);
        }
        assert_eq!(PATTERN_SIDE % PATTERN_PERIOD, 0);
        assert!(materialcraft_brush::PATTERN_PX.contains(&PATTERN_SIDE));
    }
}
