//! `mesh_faces` (since 0.9): the source polygons of the mesh parts (PT-47), which glTF cannot hold (it stores
//! triangles only). `[{"part": <index into the glb's parts>, "runs": [[faces, triangles per face], …]}, …]`; parts
//! whose triangles are their own faces are left out, so the field is absent for triangle-only meshes.

use super::fields::Obj;
use crate::json::Value;
use crate::mesh::{FaceGroups, Mesh};
use crate::paint::format::FormatError;

/// The field's value, or `None` when no part has polygons.
pub fn write(mesh: &Mesh) -> Option<Value> {
    let parts: Vec<Value> = mesh
        .parts
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.faces.is_per_triangle() && p.faces.triangle_count() == p.triangles.len())
        .map(|(i, p)| {
            let runs: Value = p.faces.runs().into_iter().map(|(n, k)| [n, k].into_iter().collect::<Value>()).collect();
            Value::object().with("part", i).with("runs", runs)
        })
        .collect();
    (!parts.is_empty()).then(|| parts.into_iter().collect())
}

/// The face groups of each listed part, by part index (applied once the mesh is read, see [`apply`]).
pub fn read(o: &mut Obj) -> Result<Vec<(usize, FaceGroups)>, FormatError> {
    let at = o.path("mesh_faces");
    let mut out = Vec::new();
    for (i, entry) in o.array("mesh_faces")?.iter().enumerate() {
        let mut e = Obj::new(entry, format!("{at}[{i}]"))?;
        let part = e.usize("part")?;
        let mut runs = Vec::new();
        for (j, r) in e.array("runs")?.iter().enumerate() {
            let pair = r.as_array().filter(|a| a.len() == 2).and_then(|a| Some((a.first()?.as_usize()?, a.get(1)?.as_usize()?)));
            runs.push(pair.ok_or_else(|| e.error("runs", format!("entry {j}: expected [faces, triangles per face]")))?);
        }
        out.push((part, FaceGroups::from_runs(runs)));
    }
    Ok(out)
}

/// Gives the read parts their face groups. Groups for a part the mesh does not have, or covering another number of
/// triangles than the part has, are dropped (the part's triangles then count as faces).
pub fn apply(mesh: &mut Mesh, faces: Vec<(usize, FaceGroups)>) {
    for (part, groups) in faces {
        if let Some(p) = mesh.parts.get_mut(part)
            && groups.triangle_count() == p.triangles.len()
        {
            p.faces = groups;
        }
    }
}
