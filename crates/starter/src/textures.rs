//! Grunge and noise textures: MaterialCraft's procedural looks ([`materialcraft::proc_textures`]) sampled into
//! seamless 16-bit grey PNGs. The sidecar records the look's id and parameters, so each one can be made again (or
//! used live as a generator with the same values).

use crate::build::Options;
use crate::image::{grey_thumb, grey16};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::json::Value;
use materialcraft::proc_textures::{self, ProcTexture};

/// One texture: the file id, display name, tags, the look and its parameters.
struct TextureRecipe {
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    look: &'static str,
    params: &'static [(&'static str, &'static str)],
}

const fn t(
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    look: &'static str,
    params: &'static [(&'static str, &'static str)],
) -> TextureRecipe {
    TextureRecipe { id, name, tags, look, params }
}

const TEXTURES: &[TextureRecipe] = &[
    t("clouds", "Clouds", &["noise", "soft"], "clouds_2", &[("seed", "1")]),
    t("perlin-noise", "Perlin noise", &["noise"], "perlin", &[("seed", "1")]),
    t("fractal-noise", "Fractal noise", &["noise"], "fractal_sum", &[("seed", "1")]),
    t("moisture", "Moisture", &["noise", "water", "stains"], "moisture", &[("seed", "1")]),
    t("anisotropic-noise", "Anisotropic noise", &["noise", "brushed", "streaks"], "anisotropic", &[("seed", "1")]),
    t("fibers", "Fibers", &["noise", "fabric", "paper"], "fibers", &[("seed", "1")]),
    t("dirt", "Dirt", &["grunge", "dirt"], "dirt_soft", &[("seed", "1")]),
    t("rust-fine", "Rust fine", &["grunge", "rust", "metal"], "rust_fine", &[("seed", "1")]),
    t("leaks", "Leaks", &["grunge", "leaks", "drips"], "leaks", &[("seed", "1")]),
    t("plaster-cracks", "Plaster cracks", &["grunge", "cracks", "wall"], "plaster_cracks", &[("seed", "1")]),
    t("concrete", "Concrete", &["grunge", "stone", "concrete"], "concrete", &[("seed", "1")]),
    t("scratches", "Scratches", &["grunge", "scratches", "wear"], "scratches", &[("seed", "1")]),
    t("smudges", "Smudges", &["grunge", "smudges", "fingerprints"], "smudges", &[("seed", "1")]),
    t("grunge-spots", "Grunge spots", &["grunge", "spots"], "grunge_spots", &[("seed", "1")]),
    t("specks", "Specks", &["grunge", "specks", "dust"], "grunge_01", &[("seed", "1")]),
    t("spatter", "Spatter", &["grunge", "spatter", "paint"], "grunge_08", &[("seed", "1")]),
    t("cracks", "Cracks", &["grunge", "cracks"], "grunge_10", &[("seed", "1")]),
    t("runs", "Runs", &["grunge", "drips", "leaks"], "grunge_15", &[("seed", "1")]),
    t("soft-spots", "Soft spots", &["spots", "soft"], "gaussian_spots_1", &[("seed", "1")]),
    t("cell-borders", "Cell borders", &["cells", "pattern"], "cells_2", &[("seed", "1")]),
    t("crystal", "Crystal", &["cells", "facets"], "crystal_1", &[("seed", "1")]),
    t("voronoi-fractal", "Voronoi fractal", &["cells", "noise"], "voronoi_fractal", &[("seed", "1")]),
];

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    TEXTURES.iter().map(|t| Box::new(Texture(t)) as Box<dyn Recipe>).collect()
}

struct Texture(&'static TextureRecipe);

impl Texture {
    fn look(&self) -> Result<ProcTexture, StarterError> {
        let params: Vec<(String, String)> = self.0.params.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        proc_textures::texture(self.0.look, &params).ok_or_else(|| StarterError::recipe(&self.path(), format!("no procedural texture {:?}", self.0.look)))
    }
}

impl Recipe for Texture {
    fn path(&self) -> String {
        format!("{}/{}.png", Usage::Texture.folder(), self.0.id)
    }

    fn make(&self, options: &Options) -> Result<Made, StarterError> {
        let look = self.look()?;
        let n = options.texture_size.max(1);
        let values: Vec<f32> = (0..n * n).map(|i| look.value(((i % n) as f32 + 0.5) / n as f32, ((i / n) as f32 + 0.5) / n as f32)).collect();
        let preset = self.0.params.iter().fold(Value::object(), |v, (k, x)| v.with(*k, *x)).with("size", n);
        let label = proc_textures::info(self.0.look).map_or(self.0.look, |i| i.label);
        let description = format!("Seamless grey texture: MaterialCraft's procedural look \"{label}\", {n} × {n} pixels, 16-bit.");
        let provenance = Provenance::Generator { generator: self.0.look.to_owned(), preset: Some(preset) };
        Ok(Made {
            bytes: grey16(n, n, &values),
            sidecar: sidecar(self.0.name, Usage::Texture, self.0.tags, &description, provenance),
            thumbnail: Some(grey_thumb(|u, v| look.value(u, v))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_look_exists_and_ids_are_unique() {
        for t in TEXTURES {
            assert!(proc_textures::info(t.look).is_some(), "{}", t.look);
            assert_eq!(TEXTURES.iter().filter(|o| o.id == t.id).count(), 1, "{}", t.id);
        }
        assert!(TEXTURES.len() >= 20);
    }
}
