//! Viewport geometry of a document: one scene part per mesh part, placed with the shared unit-size
//! fit of the whole model, using the UV set of the texture set painted on it and subdivided as that set's shader
//! instance asks (VS-09, VS-10).

use super::scene_shading::Subdivision;
use materialcraft::paint::Document;
use materialcraft::render3d;

/// A mesh part ready for `gpu3d::ScenePart`.
pub struct PartGeometry {
    pub mesh: render3d::Mesh,
    pub tangent_w: Vec<f32>,
    /// Index of the texture set painted on this part; `None` for unpainted material slots.
    pub set: Option<usize>,
    /// Index of the mesh part this is.
    pub part: usize,
}

/// Geometry of every part whose texture set is visible (`visible` and `subdivision` are indexed by texture set).
/// Parts without a texture set are always shown, never subdivided.
pub fn build(doc: &Document, visible: &[bool], subdivision: &[Subdivision]) -> Result<Vec<PartGeometry>, String> {
    doc.mesh.validate().map_err(|e| format!("The mesh is invalid: {e}"))?;
    let fit = doc.mesh.fit();
    let mut parts = Vec::new();
    for (i, part) in doc.mesh.parts.iter().enumerate() {
        let set = doc.texture_sets.iter().position(|s| s.material_slot == part.material);
        if set.is_some_and(|s| !visible.get(s).copied().unwrap_or(true)) {
            continue;
        }
        let uv_set = set.map_or(0, |s| doc.texture_sets[s].uv_set);
        let sub = set.and_then(|s| subdivision.get(s)).copied().unwrap_or_default();
        let subdivided;
        let part = if sub.levels > 0 {
            subdivided = part.subdivided(sub.levels, sub.scheme);
            &subdivided
        } else {
            part
        };
        let vp = part.to_viewport(&fit, uv_set).map_err(|e| format!("Mesh part {i} ('{}'): {e}", part.name))?;
        parts.push(PartGeometry { mesh: vp.mesh, tangent_w: vp.tangent_w, set, part: i });
    }
    Ok(parts)
}
