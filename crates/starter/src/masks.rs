//! Smart masks: presets of MaterialCraft's mask generators ([`materialcraft::generators`]), alone or with a
//! procedural texture laid over them, saved as `.mcfrag` smart masks. They hold no pixels: the generators compute
//! the mask from the baked maps of whatever they are dropped on, so the fragment's resolution is nominal. The
//! thumbnail shows the mask on the preview ball ([`crate::preview`]) and is embedded in the fragment.

use crate::build::Options;
use crate::image::THUMB_PX;
use crate::preview::Preview;
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::generators;
use materialcraft::json::Value;
use materialcraft::paint::effect::Procedural;
use materialcraft::paint::format::{SaveOptions, Thumbnail, fragment};
use materialcraft::paint::{BakedKind, BlendMode, EffectKind, Fragment, GreyBlend, Mask, MaskEffect, TextureSet};
use std::sync::OnceLock;

/// Nominal resolution of the fragments.
const RESOLUTION: u32 = 2048;

/// One effect of a mask: a generator or procedural texture id with its parameters, laid over what is below.
struct Step {
    id: &'static str,
    params: &'static [(&'static str, &'static str)],
    blend: BlendMode,
    opacity: f32,
}

const fn base(id: &'static str, params: &'static [(&'static str, &'static str)]) -> Step {
    Step { id, params, blend: BlendMode::Normal, opacity: 1.0 }
}

const fn times(id: &'static str, params: &'static [(&'static str, &'static str)]) -> Step {
    Step { id, params, blend: BlendMode::Multiply, opacity: 1.0 }
}

/// One smart mask.
struct MaskRecipe {
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    description: &'static str,
    steps: &'static [Step],
    /// Colour the thumbnail shows the mask in.
    accent: [f32; 3],
}

const RUST: [f32; 3] = [0.78, 0.36, 0.12];
const STEEL: [f32; 3] = [0.85, 0.85, 0.82];
const DIRT: [f32; 3] = [0.30, 0.22, 0.13];
const DUST: [f32; 3] = [0.82, 0.78, 0.68];
const TEAL: [f32; 3] = [0.15, 0.75, 0.66];
const STAIN: [f32; 3] = [0.62, 0.44, 0.24];

const MASKS: &[MaskRecipe] = &[
    MaskRecipe {
        id: "edge-wear",
        name: "Edge wear",
        tags: &["wear", "edges", "metal"],
        description: "Wear on exposed edges, broken up by grunge.",
        steps: &[base(generators::METAL_EDGE_WEAR, &[("seed", "1")])],
        accent: STEEL,
    },
    MaskRecipe {
        id: "edge-wear-heavy",
        name: "Edge wear heavy",
        tags: &["wear", "edges", "chipped"],
        description: "Wide, chipped wear reaching in from the edges.",
        steps: &[base(generators::METAL_EDGE_WEAR, &[("seed", "2"), ("level", "0.9"), ("grunge_amount", "0.6"), ("smoothness", "4")])],
        accent: STEEL,
    },
    MaskRecipe {
        id: "dirt",
        name: "Dirt",
        tags: &["dirt", "cavities", "occlusion"],
        description: "Dirt in occluded places and cavities, off the edges.",
        steps: &[base(generators::DIRT, &[("seed", "1")])],
        accent: DIRT,
    },
    MaskRecipe {
        id: "dirt-heavy",
        name: "Dirt heavy",
        tags: &["dirt", "cavities", "grime"],
        description: "Thick grime filling the cavities.",
        steps: &[base(generators::DIRT, &[("seed", "2"), ("level", "0.9"), ("contrast", "0.7"), ("grunge_amount", "0.5")])],
        accent: DIRT,
    },
    MaskRecipe {
        id: "dust",
        name: "Dust",
        tags: &["dust", "top", "direction"],
        description: "Dust settled on upward-facing surfaces.",
        steps: &[base(
            generators::MASK_BUILDER,
            &[
                ("seed", "1"),
                ("ao", "0.3"),
                ("curvature", "0"),
                ("normal", "1"),
                ("normal_top", "1"),
                ("level", "0.45"),
                ("contrast", "0.4"),
                ("grunge", "0.35"),
            ],
        )],
        accent: DUST,
    },
    MaskRecipe {
        id: "occlusion",
        name: "Occlusion",
        tags: &["occlusion", "cavities"],
        description: "The baked ambient occlusion as a mask: dark in the crevices.",
        steps: &[base(generators::AMBIENT_OCCLUSION, &[("ao_invert", "1"), ("ao_contrast", "0.4")])],
        accent: TEAL,
    },
    MaskRecipe {
        id: "convex-edges",
        name: "Convex edges",
        tags: &["curvature", "edges"],
        description: "Every convex edge, from the curvature over several radii.",
        steps: &[base(generators::CURVATURE, &[("curv_mode", "2"), ("curv_contrast", "0.5")])],
        accent: TEAL,
    },
    MaskRecipe {
        id: "cavities",
        name: "Cavities",
        tags: &["curvature", "cavities"],
        description: "Concave creases and cavities, from the curvature.",
        steps: &[base(generators::CURVATURE, &[("curv_mode", "3"), ("curv_contrast", "0.5")])],
        accent: TEAL,
    },
    MaskRecipe {
        id: "rust-spots",
        name: "Rust spots",
        tags: &["rust", "spots", "metal"],
        description: "Rust gathering in cavities, broken into spots by a fine rust texture.",
        steps: &[
            base(generators::DIRT, &[("seed", "3"), ("level", "0.85"), ("grunge_amount", "0.2")]),
            times("rust_fine", &[("seed", "3"), ("balance", "0.6")]),
        ],
        accent: RUST,
    },
    MaskRecipe {
        id: "leaks",
        name: "Leaks",
        tags: &["leaks", "drips", "top"],
        description: "Streaks running down from upward-facing areas.",
        steps: &[
            base(
                generators::MASK_BUILDER,
                &[("seed", "4"), ("ao", "0"), ("curvature", "0"), ("normal", "1"), ("normal_top", "1"), ("level", "0.3"), ("grunge", "0")],
            ),
            times("leaks", &[("seed", "4"), ("balance", "0.6")]),
        ],
        accent: STAIN,
    },
    MaskRecipe {
        id: "scratches",
        name: "Scratches",
        tags: &["scratches", "wear"],
        description: "Fine scratches all over, a little denser on the edges.",
        steps: &[
            base("scratches", &[("seed", "5")]),
            Step { id: generators::METAL_EDGE_WEAR, params: &[("seed", "5")], blend: BlendMode::Lighten, opacity: 0.6 },
        ],
        accent: STEEL,
    },
];

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    MASKS.iter().map(|m| Box::new(SmartMask(m)) as Box<dyn Recipe>).collect()
}

/// The preview object, made once for every mask.
fn preview() -> Result<&'static Preview, String> {
    static PREVIEW: OnceLock<Result<Preview, String>> = OnceLock::new();
    PREVIEW.get_or_init(Preview::new).as_ref().map_err(Clone::clone)
}

struct SmartMask(&'static MaskRecipe);

impl SmartMask {
    fn mask(&self) -> Mask {
        let effect = |s: &Step| {
            let p = Procedural::new(s.id).with_params(s.params.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect());
            MaskEffect::new(GreyBlend { mode: s.blend, opacity: s.opacity }, EffectKind::Generator(p))
        };
        self.0.steps.iter().fold(Mask::new(0.0), |m, s| m.with_effect(effect(s)))
    }

    /// The baked maps the mask reads (AS-05).
    fn needs(&self) -> Vec<BakedKind> {
        let mut needs: Vec<BakedKind> = Vec::new();
        for kind in self.0.steps.iter().flat_map(|s| generators::baked_maps(s.id)) {
            if !needs.contains(&kind) {
                needs.push(kind);
            }
        }
        needs
    }

    /// The effects as the sidecar's preset: id, parameters, blend and opacity of each, bottom to top.
    fn preset(&self) -> Value {
        let steps = self.0.steps.iter().map(|s| {
            let params = s.params.iter().fold(Value::object(), |v, (k, x)| v.with(*k, *x));
            Value::object().with("id", s.id).with("params", params).with("blend", s.blend.id()).with("opacity", s.opacity)
        });
        Value::object().with("effects", Value::Array(steps.collect()))
    }
}

impl Recipe for SmartMask {
    fn path(&self) -> String {
        format!("{}/{}.{}", Usage::SmartMask.folder(), self.0.id, fragment::EXTENSION)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let fail = |e: &dyn std::fmt::Display| StarterError::recipe(&self.path(), e);
        let mask = self.mask();
        let preview = preview().map_err(|e| fail(&e))?;
        let values = preview.mask_values(&mask).map_err(|e| fail(&e))?;
        let thumb = preview.ball(&values, self.0.accent, THUMB_PX);
        let mut f = Fragment::smart_mask(self.0.name, &TextureSet::new("starter", self.0.name, (RESOLUTION, RESOLUTION)), mask);
        f.needs = self.needs();
        let mut bytes = Vec::new();
        fragment::save_with(&f, &mut bytes, &SaveOptions { thumbnail: Thumbnail::Image(thumb) }).map_err(|e| fail(&e))?;
        let first = self.0.steps.first().map_or("", |s| s.id);
        let mut sc = sidecar(
            self.0.name,
            Usage::SmartMask,
            self.0.tags,
            self.0.description,
            Provenance::Generator { generator: first.to_owned(), preset: Some(self.preset()) },
        );
        sc.needs = f.needs.iter().map(|k| k.id().to_owned()).collect();
        Ok(Made { bytes, sidecar: sc, thumbnail: None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_step_names_a_registered_generator_and_ids_are_unique() {
        let r = generators::builtin();
        for m in MASKS {
            for s in m.steps {
                assert!(r.generator(s.id).is_some(), "{}: {}", m.id, s.id);
            }
            assert_eq!(MASKS.iter().filter(|o| o.id == m.id).count(), 1, "{}", m.id);
        }
        assert!(MASKS.len() >= 10);
    }
}
