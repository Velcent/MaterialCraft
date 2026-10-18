//! Brush presets translated by hand from PhotoCraft's built-in presets (`photocraft_paint::presets`, revision
//! 5f65579; PhotoCraft's own designs, MIT OR Apache-2.0), under our own names. Only the presets whose settings map
//! cleanly onto a `.mcbrush` are here; those that need a texture, a dual brush or wet edges wait for `.mcbrush` to
//! grow those sections.
//!
//! The mapping: size in pixels → a fraction of the model ([`model_size`]); pressure on opacity or flow → pressure on
//! flow; size, angle and flow jitter carry over; PhotoCraft's scatter (in radii) → position jitter (in diameters).
//! Lost on the way, and named in each description: build-up, fade, minimums of a pen dynamic, scatter count, flips,
//! colour dynamics, smoothing and a light texture.

use materialcraft::paint::brush::{BrushPreset, Jitter, PressureUse, Tip};

/// The revision of PhotoCraft the presets were translated from.
pub const PHOTOCRAFT_REVISION: &str = "5f65579";

/// One translated preset: its file id, tags, the preset, the PhotoCraft preset it comes from and what was dropped.
pub struct Translated {
    pub id: &'static str,
    pub tags: &'static [&'static str],
    pub preset: BrushPreset,
    /// Group and name of the PhotoCraft preset (for the provenance only).
    pub source: &'static str,
    /// What the translation could not keep (empty when nothing).
    pub dropped: &'static str,
}

/// A PhotoCraft brush size (pixels) as a model-relative size: 30 pixels become the default 0.08.
fn model_size(px: f32) -> f32 {
    (px * 0.08 / 30.0).clamp(0.004, 0.6)
}

/// Every translated preset.
pub fn presets() -> Vec<Translated> {
    let base = |name: &str, px: f32| BrushPreset {
        name: name.into(),
        size: model_size(px),
        hardness: 1.0,
        spacing: 0.25,
        pressure: PressureUse { size: false, flow: false },
        ..BrushPreset::default()
    };
    let tip = |id: &str| Tip::Procedural(id.into());
    let jitter = |size, flow, angle, position| Jitter { size, flow, angle, position };
    let pressure = |size, flow| PressureUse { size, flow };
    let t = |id, tags, preset, source, dropped| Translated { id, tags, preset, source, dropped };
    vec![
        // general
        t("fine-detail", &["round", "hard", "detail"], BrushPreset { spacing: 0.1, ..base("Fine detail", 5.0) }, "General / Hard Round Fine", ""),
        t("large-soft-wash", &["round", "soft", "large"], BrushPreset { hardness: 0.0, spacing: 0.15, ..base("Large soft wash", 300.0) }, "General / Soft Round Large", ""),
        t("hard-taper", &["round", "hard", "pressure"], BrushPreset { pressure: pressure(true, false), ..base("Hard taper", 30.0) }, "General / Hard Round Pressure Size", ""),
        t(
            "soft-taper",
            &["round", "soft", "pressure"],
            BrushPreset { hardness: 0.0, pressure: pressure(true, false), ..base("Soft taper", 45.0) },
            "General / Soft Round Pressure Size",
            "",
        ),
        t(
            "hard-pressure-flow",
            &["round", "hard", "pressure"],
            BrushPreset { pressure: pressure(false, true), ..base("Hard pressure flow", 30.0) },
            "General / Hard Round Pressure Opacity",
            "",
        ),
        t(
            "soft-pressure-flow",
            &["round", "soft", "pressure"],
            BrushPreset { hardness: 0.0, pressure: pressure(false, true), ..base("Soft pressure flow", 45.0) },
            "General / Soft Round Pressure Opacity",
            "",
        ),
        t(
            "firm-pressure",
            &["round", "hard", "pressure"],
            BrushPreset { hardness: 0.9, pressure: pressure(false, true), ..base("Firm pressure", 30.0) },
            "General / Hard Round Pressure Opacity and Flow",
            "the 20 % flow minimum",
        ),
        t(
            "spray-pressure",
            &["round", "soft", "spray", "pressure"],
            BrushPreset { hardness: 0.0, flow: 0.15, spacing: 0.1, pressure: pressure(false, true), ..base("Spray pressure", 120.0) },
            "General / Airbrush Pressure Flow",
            "build-up while the pen rests",
        ),
        t(
            "flat-nib",
            &["calligraphy", "flat", "ink"],
            BrushPreset { roundness: 0.2, angle: 45.0, spacing: 0.05, ..base("Flat nib", 28.0) },
            "General / Calligraphy Flat",
            "",
        ),
        // dry media
        t("hard-lead", &["pencil", "dry", "detail"], BrushPreset { spacing: 0.1, ..base("Hard lead", 3.0) }, "Dry Media / Hard Pencil", "aliased edges"),
        t(
            "bristle-drag",
            &["bristles", "dry"],
            BrushPreset { tip: tip("bristles-2"), spacing: 0.04, follow_direction: true, ..base("Bristle drag", 50.0) },
            "Dry Media / Dry Bristle",
            "the flow fading out along the stroke",
        ),
        t(
            "rake-pressure",
            &["rake", "dry", "pressure"],
            BrushPreset { tip: tip("rake-2"), spacing: 0.03, follow_direction: true, pressure: pressure(false, true), ..base("Rake pressure", 60.0) },
            "Dry Media / Rake",
            "the 25 % pressure minimum",
        ),
        // wet media
        t(
            "ink-liner",
            &["ink", "pen", "pressure"],
            BrushPreset { spacing: 0.05, pressure: pressure(true, false), ..base("Ink liner", 8.0) },
            "Wet Media / Ink Pen",
            "the 15 % size minimum and the built-in smoothing (use the lazy mouse)",
        ),
        t(
            "oil-bristle",
            &["bristles", "oil", "wet", "pressure"],
            BrushPreset { tip: tip("bristles-3"), spacing: 0.03, follow_direction: true, pressure: pressure(true, false), ..base("Oil bristle", 70.0) },
            "Wet Media / Oil Bristle",
            "the light canvas texture, colour variation and the 60 % size minimum",
        ),
        t(
            "flat-gouache",
            &["flat", "gouache", "wet"],
            BrushPreset { hardness: 0.9, roundness: 0.35, spacing: 0.05, follow_direction: true, ..base("Flat gouache", 40.0) },
            "Wet Media / Gouache Flat",
            "the flow fading out along the stroke",
        ),
        // effects
        t(
            "splatter-scatter",
            &["spatter", "scatter", "grunge"],
            BrushPreset { tip: tip("spatter-2"), spacing: 0.6, jitter: jitter(0.6, 0.0, 1.0, 0.75), ..base("Splatter scatter", 60.0) },
            "Special Effects / Spatter",
            "two dabs per step, random flips and the 20 % size minimum",
        ),
        t(
            "stipple",
            &["spatter", "stipple", "dots", "pressure"],
            BrushPreset { tip: tip("spatter-fine"), spacing: 0.5, jitter: jitter(0.0, 0.0, 1.0, 0.3), pressure: pressure(false, true), ..base("Stipple", 40.0) },
            "Special Effects / Stipple",
            "random flips",
        ),
        t(
            "confetti",
            &["scatter", "confetti"],
            BrushPreset { roundness: 0.5, spacing: 1.2, jitter: jitter(0.5, 0.0, 1.0, 1.5), ..base("Confetti", 24.0) },
            "Special Effects / Confetti",
            "three dabs per step and the random colour per dab",
        ),
        t(
            "foliage",
            &["leaf", "nature", "scatter"],
            BrushPreset { tip: tip("leaf"), spacing: 0.35, jitter: jitter(0.5, 0.0, 1.0, 0.6), ..base("Foliage", 50.0) },
            "Special Effects / Foliage",
            "two dabs per step, roundness jitter, flips, the size minimum and colour variation",
        ),
        t(
            "grass-tuft",
            &["grass", "nature", "pressure"],
            BrushPreset { tip: tip("grass-2"), spacing: 0.25, jitter: jitter(0.4, 0.0, 0.06, 0.15), pressure: pressure(true, false), ..base("Grass tuft", 70.0) },
            "Special Effects / Grass",
            "flips, the 40 % size minimum and colour variation",
        ),
        t(
            "glints",
            &["sparkle", "stars", "scatter"],
            BrushPreset { tip: tip("sparkle"), spacing: 1.5, jitter: jitter(0.8, 0.5, 0.0, 1.25), ..base("Glints", 40.0) },
            "Special Effects / Sparkle",
            "the 15 % size minimum",
        ),
        t(
            "sponge-stamp",
            &["sponge", "grunge"],
            BrushPreset { tip: tip("sponge-3"), spacing: 0.45, jitter: jitter(0.0, 0.3, 1.0, 0.0), ..base("Sponge stamp", 60.0) },
            "Special Effects / Sponge Texture",
            "",
        ),
    ]
}
