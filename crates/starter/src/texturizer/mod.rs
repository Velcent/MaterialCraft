//! Relief textures: the height fields of PhotoCraft's Texturizer filter ([`field`], ported) as seamless 16-bit grey
//! PNGs, for height detail (brick, burlap, canvas, sandstone, blocks, frosted glass, tiny lenses).

mod field;

use crate::build::Options;
use crate::image::{grey_thumb, grey16};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use field::{Field, TILE};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::json::Value;

/// Pixels per field unit: PhotoCraft's Texturizer at 200 % scaling.
const SCALE: f32 = 2.0;

/// Side of every relief texture, in pixels: one whole tile.
const SIDE: usize = (TILE * SCALE) as usize;

/// The generator the relief textures record in their provenance.
const GENERATOR: &str = "photocraft.texturizer";

/// One relief texture: file id, display name, field and tags.
struct ReliefRecipe {
    id: &'static str,
    name: &'static str,
    field: Field,
    tags: &'static [&'static str],
}

const fn r(id: &'static str, name: &'static str, field: Field, tags: &'static [&'static str]) -> ReliefRecipe {
    ReliefRecipe { id, name, field, tags }
}

const RELIEFS: &[ReliefRecipe] = &[
    r("relief-brick", "Brick relief", Field::Brick, &["relief", "height", "brick", "wall"]),
    r("relief-burlap", "Burlap relief", Field::Burlap, &["relief", "height", "fabric", "burlap"]),
    r("relief-canvas", "Canvas relief", Field::Canvas, &["relief", "height", "fabric", "canvas"]),
    r("relief-sandstone", "Sandstone relief", Field::Sandstone, &["relief", "height", "stone", "sandstone"]),
    r("relief-blocks", "Blocks relief", Field::Blocks, &["relief", "height", "blocks", "tiles"]),
    r("relief-frosted", "Frosted relief", Field::Frosted, &["relief", "height", "glass", "frosted"]),
    r("relief-tiny-lens", "Tiny lens relief", Field::TinyLens, &["relief", "height", "lens", "bumps"]),
];

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    RELIEFS.iter().map(|r| Box::new(Relief(r)) as Box<dyn Recipe>).collect()
}

struct Relief(&'static ReliefRecipe);

impl Recipe for Relief {
    fn path(&self) -> String {
        format!("{}/{}.png", Usage::Texture.folder(), self.0.id)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let n = SIDE;
        let f = self.0.field;
        let values: Vec<f32> = (0..n * n).map(|i| f.height((i % n) as f32 / SCALE, (i / n) as f32 / SCALE)).collect();
        // the thumbnail shows a quarter of the tile, so the relief stays readable
        let at = |u: f32, v: f32| f.height(u * TILE / 4.0, v * TILE / 4.0);
        let preset = Value::object().with("field", f.id()).with("scaling", (SCALE * 100.0) as u64).with("size", n);
        let description = format!("Seamless height texture: the {} field of PhotoCraft's Texturizer at 200 %, {n} × {n} pixels, 16-bit.", f.id());
        let mut tags = self.0.tags.to_vec();
        tags.push("photocraft");
        Ok(Made {
            bytes: grey16(n, n, &values),
            sidecar: sidecar(self.0.name, Usage::Texture, &tags, &description, Provenance::Generator { generator: GENERATOR.into(), preset: Some(preset) }),
            thumbnail: Some(grey_thumb(at)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_the_side_is_one_tile() {
        for r in RELIEFS {
            assert_eq!(RELIEFS.iter().filter(|o| o.id == r.id).count(), 1, "{}", r.id);
        }
        assert_eq!(SIDE as f32, TILE * SCALE);
        assert_eq!(RELIEFS.len(), 7);
    }
}
