//! Which texture set and mesh map a baked map file is for, from its name: `<texture set name>_<map>.<ext>`, where
//! `<map>` is one of [`MAP_NAMES`] (e.g. `Body_ambient_occlusion.png`). The set name is the material's name, matched
//! exactly first and then ignoring case.

use super::material_sets::PlannedSet;
use crate::paint::BakedKind;
use std::path::{Path, PathBuf};

/// The map names files end in, with the baked map each one gives.
pub const MAP_NAMES: [(&str, BakedKind); 7] = [
    ("ambient_occlusion", BakedKind::AmbientOcclusion),
    ("curvature", BakedKind::Curvature),
    ("normal", BakedKind::Normal),
    ("world_space_normals", BakedKind::WorldSpaceNormal),
    ("id", BakedKind::Id),
    ("position", BakedKind::Position),
    ("thickness", BakedKind::Thickness),
];

/// A file matched to a texture set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assigned {
    pub file: PathBuf,
    /// The texture set's id.
    pub set: String,
    pub kind: BakedKind,
}

/// A file that matched no texture set or map, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unassigned {
    pub file: PathBuf,
    pub reason: String,
}

/// The result of matching a list of files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Assignment {
    pub assigned: Vec<Assigned>,
    pub unassigned: Vec<Unassigned>,
}

/// The map a file stem names and the set name before it: `("Body", Normal)` for `Body_normal`.
pub fn split_name(stem: &str) -> Option<(&str, BakedKind)> {
    let lower = stem.to_ascii_lowercase();
    let mut names: Vec<&(&str, BakedKind)> = MAP_NAMES.iter().collect();
    // longest first, so `…_world_space_normals` is not read as something shorter
    names.sort_by_key(|(n, _)| std::cmp::Reverse(n.len()));
    names.into_iter().find_map(|(name, kind)| {
        let cut = lower.len().checked_sub(name.len() + 1)?;
        let tail = lower.get(cut..)?;
        (tail.strip_prefix('_') == Some(name)).then(|| stem.get(..cut).map(|set| (set, kind.clone()))).flatten().filter(|(set, _)| !set.is_empty())
    })
}

/// Matches every file to a set of `sets`; a second file for the same set and map is left out.
pub fn assign(files: &[PathBuf], sets: &[PlannedSet]) -> Assignment {
    let mut out = Assignment::default();
    for file in files {
        match assign_one(file, sets) {
            Ok(a) if out.assigned.iter().any(|b| b.set == a.set && b.kind == a.kind) => {
                out.unassigned.push(Unassigned { file: file.clone(), reason: format!("another file already gives this set's {} map", a.kind.id()) })
            }
            Ok(a) => out.assigned.push(a),
            Err(reason) => out.unassigned.push(Unassigned { file: file.clone(), reason }),
        }
    }
    out
}

fn assign_one(file: &Path, sets: &[PlannedSet]) -> Result<Assigned, String> {
    let stem = file.file_stem().and_then(|s| s.to_str()).ok_or("the file name is not readable text")?;
    let names = || MAP_NAMES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ");
    let (set_name, kind) = split_name(stem).ok_or_else(|| format!("the name does not end in _<map> (one of {})", names()))?;
    let set = sets
        .iter()
        .find(|s| s.name == set_name)
        .or_else(|| sets.iter().find(|s| s.name.eq_ignore_ascii_case(set_name)))
        .ok_or_else(|| format!("no texture set is named \"{set_name}\""))?;
    Ok(Assigned { file: file.to_path_buf(), set: set.id.clone(), kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(id: &str, name: &str) -> PlannedSet {
        PlannedSet { id: id.into(), name: name.into(), slot: 0 }
    }

    #[test]
    fn names_split_into_set_and_map() {
        assert_eq!(split_name("Body_normal"), Some(("Body", BakedKind::Normal)));
        assert_eq!(split_name("Body_world_space_normals"), Some(("Body", BakedKind::WorldSpaceNormal)));
        assert_eq!(split_name("Iron_Gate_Ambient_Occlusion"), Some(("Iron_Gate", BakedKind::AmbientOcclusion)));
        assert_eq!(split_name("Body_id"), Some(("Body", BakedKind::Id)));
        assert_eq!(split_name("_normal"), None);
        assert_eq!(split_name("normal"), None);
        assert_eq!(split_name("Body_roughness"), None);
    }

    #[test]
    fn files_are_assigned_by_name() {
        let sets = [set("body", "Body"), set("lid", "Lid"), set("lid_2", "lid")];
        let files: Vec<PathBuf> = [
            "maps/Body_normal.png",
            "maps/body_curvature.exr",
            "maps/lid_thickness.png",
            "maps/Lid_position.png",
            "Body_normal.exr",
            "Head_normal.png",
            "Body.png",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect();
        let a = assign(&files, &sets);
        let got: Vec<(&str, &str)> = a.assigned.iter().map(|x| (x.set.as_str(), x.kind.id())).collect();
        assert_eq!(got, [("body", "normal"), ("body", "curvature"), ("lid_2", "thickness"), ("lid", "position")]);
        let reasons: Vec<&str> = a.unassigned.iter().map(|u| u.reason.as_str()).collect();
        assert_eq!(reasons.len(), 3);
        assert!(reasons[0].contains("already"));
        assert!(reasons[1].contains("\"Head\""));
        assert!(reasons[2].contains("_<map>"));
    }
}
