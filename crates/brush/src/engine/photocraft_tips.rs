//! PhotoCraft's procedural brush tips (`photocraft_paint::procedural`): generated, seeded greyscale tips (chalk,
//! spatter, bristles …) written for PhotoCraft from scratch, no bundled brush files. The starter library ships them
//! as alphas (ADR 0008 rm-starter-scope), and a preset's procedural tip ([`Tip::Procedural`]) is rendered here
//! ([`procedural_tip_alpha`]), so it paints, and replays, without any image. Besides [`super::photocraft`], this is the only
//! file naming `photocraft_paint`.
//!
//! [`Tip::Procedural`]: materialcraft::paint::brush::Tip::Procedural

use super::TipAlpha;
use photocraft_paint::GrayTile;
use photocraft_paint::procedural as pc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The generator an alpha made from a procedural tip records in its provenance, with the tip's id as `tip` in its
/// preset (the starter library's alphas): such an alpha can be the tip itself.
pub const PROCEDURAL_TIP_GENERATOR: &str = "photocraft-paint.procedural";

/// Side of a procedural tip as a preset's tip: the starter library's alphas have this size too, so a brush with the
/// tip and one with its alpha paint the same.
pub const PROCEDURAL_TIP_PX: u32 = 256;

/// The procedural tip `id` as a stamp tip, [`PROCEDURAL_TIP_PX`] square; rendered once per id and process. `None` for an unknown
/// id.
pub fn procedural_tip_alpha(id: &str) -> Option<TipAlpha> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<TipAlpha>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(t) = cache.lock().ok().and_then(|c| c.get(id).cloned()) {
        return Some((*t).clone());
    }
    let tip = TipAlpha::new(PROCEDURAL_TIP_PX, PROCEDURAL_TIP_PX, procedural_tip(id, PROCEDURAL_TIP_PX)?)?;
    if let Ok(mut c) = cache.lock() {
        c.insert(id.to_string(), Arc::new(tip.clone()));
    }
    Some(tip)
}

/// One procedural tip: its id, a display name and how PhotoCraft makes it (function and fixed arguments, for the
/// asset's provenance).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TipInfo {
    pub id: &'static str,
    pub label: &'static str,
    /// The `photocraft_paint::procedural` function and its arguments other than the size.
    pub recipe: &'static str,
}

const fn tip(id: &'static str, label: &'static str, recipe: &'static str) -> TipInfo {
    TipInfo { id, label, recipe }
}

/// Every procedural tip, in menu order: the nine first tips (seed 1), then seeded variants (the seeds PhotoCraft's own
/// presets paint with, and a few more). Ids are never changed or reused: recorded strokes name their tip by id.
pub const PROCEDURAL_TIPS: &[TipInfo] = &[
    tip("chalk", "Chalk", "chalk_tip(seed 1)"),
    tip("charcoal", "Charcoal", "charcoal_tip(seed 1)"),
    tip("bristles", "Bristles", "bristle_tip(seed 1)"),
    tip("spatter", "Spatter", "spatter_tip(seed 1, drops 18)"),
    tip("sponge", "Sponge", "sponge_tip(seed 1)"),
    tip("rake", "Rake", "rake_tip(seed 1, bristles 7)"),
    tip("grass", "Grass", "grass_tip(seed 1, blades 9)"),
    tip("leaf", "Leaf", "leaf_tip()"),
    tip("sparkle", "Sparkle", "star_tip()"),
    // seeded variants (leaf and sparkle take no seed)
    tip("chalk-2", "Chalk 2", "chalk_tip(seed 7)"),
    tip("chalk-3", "Chalk 3", "chalk_tip(seed 31)"),
    tip("charcoal-2", "Charcoal 2", "charcoal_tip(seed 4)"),
    tip("charcoal-3", "Charcoal 3", "charcoal_tip(seed 9)"),
    tip("bristles-2", "Bristles 2", "bristle_tip(seed 5)"),
    tip("bristles-3", "Bristles 3", "bristle_tip(seed 41)"),
    tip("spatter-2", "Spatter 2", "spatter_tip(seed 11, drops 14)"),
    tip("spatter-fine", "Spatter fine", "spatter_tip(seed 77, drops 40)"),
    tip("spatter-grain", "Spatter grain", "spatter_tip(seed 23, drops 20)"),
    tip("sponge-2", "Sponge 2", "sponge_tip(seed 8)"),
    tip("sponge-3", "Sponge 3", "sponge_tip(seed 19)"),
    tip("rake-2", "Rake 2", "rake_tip(seed 3, bristles 7)"),
    tip("rake-wide", "Rake wide", "rake_tip(seed 2, bristles 5)"),
    tip("rake-fine", "Rake fine", "rake_tip(seed 4, bristles 12)"),
    tip("grass-2", "Grass 2", "grass_tip(seed 6, blades 11)"),
    tip("grass-sparse", "Grass sparse", "grass_tip(seed 2, blades 5)"),
    tip("grass-dense", "Grass dense", "grass_tip(seed 3, blades 16)"),
];

/// The tip `id` rendered `side` × `side`, row by row, 0..1 (16-bit steps); `None` for an unknown id or side 0.
pub fn procedural_tip(id: &str, side: u32) -> Option<Vec<f32>> {
    if side == 0 {
        return None;
    }
    let tile: GrayTile = match id {
        "chalk" => pc::chalk_tip(side, 1),
        "charcoal" => pc::charcoal_tip(side, 1),
        "bristles" => pc::bristle_tip(side, 1),
        "spatter" => pc::spatter_tip(side, 1, 18),
        "sponge" => pc::sponge_tip(side, 1),
        "rake" => pc::rake_tip(side, 1, 7),
        "grass" => pc::grass_tip(side, 1, 9),
        "leaf" => pc::leaf_tip(side),
        "sparkle" => pc::star_tip(side),
        "chalk-2" => pc::chalk_tip(side, 7),
        "chalk-3" => pc::chalk_tip(side, 31),
        "charcoal-2" => pc::charcoal_tip(side, 4),
        "charcoal-3" => pc::charcoal_tip(side, 9),
        "bristles-2" => pc::bristle_tip(side, 5),
        "bristles-3" => pc::bristle_tip(side, 41),
        "spatter-2" => pc::spatter_tip(side, 11, 14),
        "spatter-fine" => pc::spatter_tip(side, 77, 40),
        "spatter-grain" => pc::spatter_tip(side, 23, 20),
        "sponge-2" => pc::sponge_tip(side, 8),
        "sponge-3" => pc::sponge_tip(side, 19),
        "rake-2" => pc::rake_tip(side, 3, 7),
        "rake-wide" => pc::rake_tip(side, 2, 5),
        "rake-fine" => pc::rake_tip(side, 4, 12),
        "grass-2" => pc::grass_tip(side, 6, 11),
        "grass-sparse" => pc::grass_tip(side, 2, 5),
        "grass-dense" => pc::grass_tip(side, 3, 16),
        _ => return None,
    };
    (tile.is_valid() && tile.width == side && tile.height == side).then(|| tile.to_f32())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_tip_renders_and_is_repeatable() {
        for t in PROCEDURAL_TIPS {
            let a = procedural_tip(t.id, 32).unwrap();
            assert_eq!(a.len(), 32 * 32, "{}", t.id);
            assert!(a.iter().any(|v| *v > 0.1), "{} is empty", t.id);
            assert_eq!(Some(a), procedural_tip(t.id, 32));
        }
        assert!(procedural_tip("nope", 32).is_none());
        for (i, t) in PROCEDURAL_TIPS.iter().enumerate() {
            assert!(PROCEDURAL_TIPS.iter().skip(i + 1).all(|o| o.id != t.id), "{} twice", t.id);
        }
    }

    #[test]
    fn the_first_tips_keep_their_ids_and_variants_differ() {
        // recorded strokes name their tip by id: these never change
        let first: Vec<&str> = PROCEDURAL_TIPS.iter().take(9).map(|t| t.id).collect();
        assert_eq!(first, ["chalk", "charcoal", "bristles", "spatter", "sponge", "rake", "grass", "leaf", "sparkle"]);
        assert_ne!(procedural_tip("chalk", 32), procedural_tip("chalk-2", 32));
        assert_ne!(procedural_tip("rake-wide", 32), procedural_tip("rake-fine", 32));
        assert!(procedural_tip("chalk", 0).is_none());
    }
}
