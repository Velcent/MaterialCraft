//! PhotoCraft's procedural brush tips (`photocraft_paint::procedural`): generated, seeded greyscale tips (chalk,
//! spatter, bristles …) written for PhotoCraft from scratch, no bundled brush files. The starter library ships them
//! as alphas (ADR 0008 rm-starter-scope). Besides [`super::photocraft`], this is the only file naming
//! `photocraft_paint`.

use photocraft_paint::GrayTile;
use photocraft_paint::procedural as pc;

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

/// Every procedural tip, in menu order.
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
        assert!(procedural_tip("chalk", 0).is_none());
    }
}
