//! Viewport geometry of a document: one scene part per mesh part, placed with the shared unit-size
//! fit of the whole model and using the UV set of the texture set painted on it.

use materialcraft::paint::Document;
use materialcraft::render3d;

/// A mesh part ready for `gpu3d::ScenePart`.
pub struct PartGeometry {
    pub mesh: render3d::Mesh,
    pub tangent_w: Vec<f32>,
    /// Index of the texture set painted on this part; `None` for unpainted material slots.
    pub set: Option<usize>,
}

/// Geometry of every part whose texture set is visible (`visible` is indexed by texture set).
/// Parts without a texture set are always shown.
pub fn build(doc: &Document, visible: &[bool]) -> Result<Vec<PartGeometry>, String> {
    doc.mesh
        .validate()
        .map_err(|e| format!("The mesh is invalid: {e}"))?;
    let fit = doc.mesh.fit();
    let mut parts = Vec::new();
    for (i, part) in doc.mesh.parts.iter().enumerate() {
        let set = doc
            .texture_sets
            .iter()
            .position(|s| s.material_slot == part.material);
        if set.is_some_and(|s| !visible.get(s).copied().unwrap_or(true)) {
            continue;
        }
        let uv_set = set.map_or(0, |s| doc.texture_sets[s].uv_set);
        let vp = part
            .to_viewport(&fit, uv_set)
            .map_err(|e| format!("Mesh part {i} ('{}'): {e}", part.name))?;
        parts.push(PartGeometry {
            mesh: vp.mesh,
            tangent_w: vp.tangent_w,
            set,
        });
    }
    Ok(parts)
}
