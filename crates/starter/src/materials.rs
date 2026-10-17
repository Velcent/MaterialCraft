//! Materials: node graphs in `content/materials/*.mcg`, published as `.mca` packages ([`Graph::publish`]). Each
//! graph follows the node editor's "Metallic Roughness" template (its outputs, [`TEMPLATE_OUTPUTS`]) and is built
//! from the designer side's nodes; the thumbnail is the tray's material ball ([`material_fill::ball`]).

use crate::build::Options;
use crate::image::{THUMB_PX, rgb8};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{Provenance, Usage};
use materialcraft::graph::{self, Graph};
use materialcraft::json::Value;
use materialcraft::material_fill::{self, Material};

/// The outputs of the node editor's "Metallic Roughness" template, which every starter material has.
const TEMPLATE_OUTPUTS: [&str; 6] = ["basecolor", "normal", "roughness", "metallic", "height", "ambient_occlusion"];

/// Size the material graphs are evaluated at for their thumbnail.
const THUMB_EVAL: usize = 128;

/// One material: file id, display name, tags, a description and the graph text.
struct MaterialRecipe {
    id: &'static str,
    name: &'static str,
    tags: &'static [&'static str],
    description: &'static str,
    graph: &'static str,
    /// Where the graph comes from, for the provenance: its file, or the constant it is.
    source: &'static str,
}

macro_rules! material {
    ($id:literal, $name:literal, [$($tag:literal),*], $desc:literal) => {
        MaterialRecipe {
            id: $id,
            name: $name,
            tags: &[$($tag),*],
            description: $desc,
            graph: include_str!(concat!("../content/materials/", $id, ".mcg")),
            source: concat!("crates/starter/content/materials/", $id, ".mcg"),
        }
    };
    ($id:literal, $name:literal, [$($tag:literal),*], $desc:literal, $graph:expr) => {
        MaterialRecipe { id: $id, name: $name, tags: &[$($tag),*], description: $desc, graph: $graph, source: stringify!($graph) }
    };
}

const MATERIALS: &[MaterialRecipe] = &[
    // metals
    material!("steel-brushed", "Brushed steel", ["metal", "steel", "brushed"], "Steel brushed in one direction, with fine streaks in the roughness."),
    material!("chrome", "Chrome", ["metal", "chrome", "polished"], "Polished chrome with faint smudges."),
    material!("copper", "Copper", ["metal", "copper"], "Bare copper with soft tarnish."),
    material!("gold", "Gold", ["metal", "gold", "polished"], "Polished yellow gold."),
    material!("iron-rusted", "Rusted iron", ["metal", "iron", "rust", "worn"], "Dark iron with patches of rust."),
    material!("aluminium-cast", "Cast aluminium", ["metal", "aluminium", "rough"], "Rough sand-cast aluminium."),
    // woods
    material!("wood-oak", "Oak", ["wood", "oak"], "Light oak with long grain."),
    material!("wood-walnut", "Walnut", ["wood", "walnut", "dark"], "Dark walnut with wavy grain."),
    material!("wood-planks", "Wood planks", ["wood", "planks", "floor"], "Pine floor planks with gaps."),
    material!("wood-painted", "Painted planks", ["wood", "paint", "planks", "worn"], "Planks with peeling blue paint."),
    // stones
    material!("brick-wall", "Brick wall", ["stone", "brick", "wall"], "Red brick wall with mortar (the node editor's example graph).", graph::EXAMPLE),
    material!("cobblestone", "Cobblestones", ["stone", "cobblestone", "ground"], "Rounded cobblestones in dark joints."),
    material!("concrete", "Concrete", ["stone", "concrete"], "Grey cast concrete with pores and stains."),
    material!("marble", "Marble", ["stone", "marble", "polished"], "Polished white marble with grey veins."),
    material!("slate-tiles", "Slate tiles", ["stone", "slate", "tiles"], "Dark slate tiles with chipped edges."),
    // plastics
    material!("plastic-glossy", "Glossy plastic", ["plastic", "glossy"], "Smooth red plastic."),
    material!("plastic-matte", "Matte plastic", ["plastic", "matte"], "Fine-textured grey plastic."),
    material!("rubber", "Rubber", ["plastic", "rubber"], "Black rubber with a soft sheen."),
    // fabrics and leather
    material!("fabric-woven", "Woven fabric", ["fabric", "cloth", "woven"], "Coarse woven cloth."),
    material!("denim", "Denim", ["fabric", "denim", "cloth"], "Blue twill denim."),
    material!("leather", "Leather", ["leather", "fabric"], "Brown grained leather."),
    // paints and coatings
    material!("car-paint", "Car paint", ["paint", "glossy", "metallic"], "Deep red metallic car paint."),
    material!("paint-chipped", "Chipped paint", ["paint", "metal", "worn"], "Green paint chipped down to bare steel."),
    material!("plaster", "Plaster", ["paint", "plaster", "wall"], "Painted plaster wall with soft bumps."),
    // ground
    material!("mud", "Mud", ["ground", "mud", "wet"], "Wet brown mud."),
    material!("sand", "Sand", ["ground", "sand"], "Fine sand with wind ripples."),
    material!("gravel", "Gravel", ["ground", "gravel", "stone"], "Loose grey gravel."),
    material!("moss", "Moss", ["ground", "moss", "organic"], "Soft green moss."),
    material!("asphalt", "Asphalt", ["ground", "asphalt", "road"], "Dark road asphalt with stones."),
    // ceramics
    material!("ceramic-tiles", "Ceramic tiles", ["ceramic", "tiles", "glossy"], "White glazed tiles with grout."),
];

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    MATERIALS.iter().map(|m| Box::new(MaterialAsset(m)) as Box<dyn Recipe>).collect()
}

struct MaterialAsset(&'static MaterialRecipe);

impl MaterialAsset {
    /// The published package: the graph, named after the asset, flattened by [`Graph::publish`].
    fn published(&self) -> Result<String, StarterError> {
        let fail = |e: &dyn std::fmt::Display| StarterError::recipe(&self.path(), e);
        let mut graph = Graph::parse(self.0.graph).map_err(|e| fail(&e))?;
        let outputs = graph.output_nodes();
        if let Some(missing) = TEMPLATE_OUTPUTS.iter().find(|o| !outputs.iter().any(|(usage, _, _)| usage == *o)) {
            return Err(fail(&format!("the graph has no {missing} output")));
        }
        graph.meta.insert("identifier".into(), self.0.id.to_owned());
        graph.meta.insert("label".into(), self.0.name.to_owned());
        graph.publish().map_err(|e| fail(&e))
    }
}

impl Recipe for MaterialAsset {
    fn path(&self) -> String {
        format!("{}/{}.mca", Usage::Material.folder(), self.0.id)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let text = self.published()?;
        let material = Material::from_text(&text, &[]).map_err(|e| StarterError::recipe(&self.path(), e))?;
        let ball = material_fill::ball(&material, THUMB_EVAL, THUMB_PX).map_err(|e| StarterError::recipe(&self.path(), e))?;
        let provenance = Provenance::Graph { graph: self.0.source.to_owned(), parameters: Some(Value::object().with("template", "Metallic Roughness")) };
        Ok(Made {
            bytes: text.into_bytes(),
            sidecar: sidecar(self.0.name, Usage::Material, self.0.tags, self.0.description, provenance),
            thumbnail: Some(rgb8(&ball)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_graph_parses_and_has_the_template_outputs() {
        for m in MATERIALS {
            let g = Graph::parse(m.graph).unwrap_or_else(|e| panic!("{}: {e}", m.id));
            let outs: Vec<String> = g.output_nodes().into_iter().map(|(usage, _, _)| usage).collect();
            for o in TEMPLATE_OUTPUTS {
                assert!(outs.iter().any(|u| u == o), "{} has no {o} output", m.id);
            }
            assert_eq!(MATERIALS.iter().filter(|x| x.id == m.id).count(), 1, "{}", m.id);
        }
        assert!(MATERIALS.len() >= 30);
    }
}
