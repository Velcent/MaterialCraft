//! [`MeshFinder`]: finds a high-poly mesh file by name near a project. Painter records the path on the machine
//! it ran on (`file:///D:/…/Sload HP.OBJ`), so only the file name is used, searched below the project's own
//! folder (never above it). Only `.obj` and `.fbx` names are ever matched; nothing else is opened.

use std::path::{Path, PathBuf};

/// Folder levels searched below the project folder.
const DEPTH: usize = 4;
const MESH_EXTENSIONS: &[&str] = &["obj", "fbx"];

pub struct MeshFinder<'a> {
    project_dir: &'a Path,
}

impl<'a> MeshFinder<'a> {
    pub fn new(project_dir: &'a Path) -> MeshFinder<'a> {
        MeshFinder { project_dir }
    }

    /// The first file named `name` (case-insensitive), nearest first.
    pub fn find(&self, name: &str) -> Option<PathBuf> {
        if !is_mesh_name(name) {
            return None;
        }
        let wanted = name.to_lowercase();
        let mut found = Vec::new();
        search(self.project_dir, &wanted, DEPTH, &mut found);
        found.sort_by_key(|p| (p.components().count(), p.clone()));
        found.into_iter().next()
    }
}

pub fn is_mesh_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MESH_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

fn search(dir: &Path, wanted: &str, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() && depth > 0 {
            search(&path, wanted, depth - 1, found);
        } else if kind.is_file() && entry.file_name().to_string_lossy().to_lowercase() == wanted {
            found.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_by_name_nearest_first_and_only_meshes() {
        let root = std::env::temp_dir().join(format!("mcparity-finder-{}", std::process::id()));
        let project = root.join("proj");
        std::fs::create_dir_all(project.join("sub/deeper")).unwrap();
        std::fs::create_dir_all(root.join("other")).unwrap();
        // Above the project folder: never found.
        std::fs::write(project.join("sub/deeper/Vase HP.obj"), "v 0 0 0").unwrap();
        std::fs::write(project.join("sub/vase hp.OBJ"), "v 0 0 0").unwrap();
        std::fs::write(root.join("other/Far.fbx"), "").unwrap();
        std::fs::write(project.join("notes.txt"), "").unwrap();
        let finder = MeshFinder::new(&project);
        assert_eq!(finder.find("VASE HP.obj"), Some(project.join("sub/vase hp.OBJ")));
        assert_eq!(finder.find("Far.fbx"), None);
        assert_eq!(finder.find("notes.txt"), None);
        assert_eq!(finder.find("missing.obj"), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
