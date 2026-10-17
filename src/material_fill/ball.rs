//! [`ball`]: a material shown on a lit ball, the picture the asset tray and the starter library use for a
//! material. The graph is evaluated small and drawn by the software renderer ([`crate::render3d`]); the same
//! material gives the same pixels.

use super::Material;
use crate::render3d::{self, Maps, View};
use crate::tex::Tex;
use std::collections::BTreeMap;

/// `material` evaluated at `eval` pixels and drawn on a ball `side` pixels square (display values).
pub fn ball(material: &Material, eval: usize, side: usize) -> Result<Tex, String> {
    let graph = material.graph();
    let mut outputs = graph.evaluate(Some(eval)).map_err(|e| e.to_string())?;
    let mut by_usage: BTreeMap<String, Tex> = BTreeMap::new();
    for (usage, ident, _) in graph.output_nodes() {
        if !by_usage.contains_key(&usage)
            && let Some(t) = outputs.remove(&ident)
        {
            by_usage.insert(usage, t);
        }
    }
    let s = eval;
    let (half, flat, one, zero) =
        (Tex::solid(s, s, [0.5, 0.5, 0.5, 1.0]), Tex::solid(s, s, [0.5, 0.5, 1.0, 1.0]), Tex::solid(s, s, [1.0; 4]), Tex::solid(s, s, [0.0, 0.0, 0.0, 1.0]));
    let get = |names: &[&str]| names.iter().find_map(|n| by_usage.get(*n));
    let maps = Maps {
        base: get(&["basecolor", "diffuse"]).unwrap_or(&half),
        normal: get(&["normal"]).unwrap_or(&flat),
        rough: get(&["roughness"]).unwrap_or(&half),
        metal: get(&["metallic"]).unwrap_or(&zero),
        ao: get(&["ambient_occlusion"]).unwrap_or(&one),
        emissive: get(&["emissive"]),
        height: get(&["height"]),
    };
    let view = View { tess: 48, relief: render3d::RELIEF_BUMP, ..View::default() };
    Ok(render3d::render(&maps, &view, side, side))
}
