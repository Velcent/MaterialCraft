//! [`MaterialSets`]: one texture set per material slot the mesh's parts use (OBJ `usemtl` names, glTF materials), in
//! slot order. Parts of different objects that share a material share its set; different materials may overlap in UV
//! space, since each set has its own textures.

use crate::mesh::Mesh;

/// A texture set to make.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSet {
    /// Unique within the document, made from the name.
    pub id: String,
    /// The material's name (also what baked map files are matched against).
    pub name: String,
    /// Index into the mesh's material slots.
    pub slot: usize,
}

/// The texture sets of a mesh.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaterialSets {
    pub sets: Vec<PlannedSet>,
}

impl MaterialSets {
    /// One set per material slot used by a part with triangles; slots nothing uses get none.
    pub fn of(mesh: &Mesh) -> MaterialSets {
        let mut used: Vec<usize> = mesh.parts.iter().filter(|p| !p.triangles.is_empty()).map(|p| p.material).collect();
        used.sort_unstable();
        used.dedup();
        let mut sets: Vec<PlannedSet> = Vec::with_capacity(used.len());
        for slot in used {
            let name = mesh.materials.get(slot).map(|n| n.trim()).filter(|n| !n.is_empty()).map_or_else(|| format!("material{slot}"), str::to_string);
            let id = unique_id(&slug(&name), |id| sets.iter().any(|s| s.id == id));
            sets.push(PlannedSet { id, name, slot });
        }
        MaterialSets { sets }
    }

    /// Names of the materials whose parts lack the UV set textures are painted on.
    pub fn without_uvs(&self, mesh: &Mesh) -> Vec<String> {
        self.sets
            .iter()
            .filter(|s| mesh.parts.iter().any(|p| p.material == s.slot && !p.triangles.is_empty() && p.uv_sets.is_empty()))
            .map(|s| s.name.clone())
            .collect()
    }
}

/// Lowercase letters, digits and `_`; anything else becomes `_`; never empty.
fn slug(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "set".to_string() } else { s }
}

/// `base`, or `base_2`, `base_3`, … if taken.
fn unique_id(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..).map(|n| format!("{base}_{n}")).find(|id| !taken(id)).unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::MeshPart;

    fn part(material: usize, uvs: bool) -> MeshPart {
        let positions = vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv_sets = if uvs { vec![vec![[0.0; 2]; 3]] } else { Vec::new() };
        MeshPart { material, positions, uv_sets, triangles: vec![[0, 1, 2]], ..MeshPart::default() }
    }

    #[test]
    fn one_set_per_used_material_with_unique_ids() {
        let mesh = Mesh {
            materials: vec!["Body Paint".into(), "unused".into(), "body-paint".into(), "".into()],
            parts: vec![part(2, true), part(0, true), part(0, true), part(3, false)],
        };
        let sets = MaterialSets::of(&mesh);
        let ids: Vec<&str> = sets.sets.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["body_paint", "body_paint_2", "material3"]);
        assert_eq!(sets.sets.iter().map(|s| s.slot).collect::<Vec<_>>(), [0, 2, 3]);
        assert_eq!(sets.without_uvs(&mesh), ["material3"]);
    }
}
