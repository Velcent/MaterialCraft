//! [`MeshFinder`]: finds a high-poly mesh file near a document or project. Imported projects record the path on the
//! machine they were made on (`D:/…/The Sload/Sload HP.OBJ`), so the file is searched by name below the document's
//! own folder (never above it); among several files of that name the one whose folders match the recorded path's
//! last folders wins, then the nearest. Only `.obj` and `.fbx` names are ever matched; nothing else is opened.
//! Shared by the Paint screen's bake queue and `mcparity`.

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

    /// The file `recorded` names (a file name, or a path from any machine with `/` or `\\` separators), matched
    /// case-insensitively: the candidate sharing most of the recorded path's last folders, then the nearest.
    pub fn find(&self, recorded: &str) -> Option<PathBuf> {
        let parts = path_parts(recorded);
        let (name, folders) = parts.split_last()?;
        if !is_mesh_name(name) {
            return None;
        }
        let mut found = Vec::new();
        search(self.project_dir, name, DEPTH, &mut found);
        found.sort_by_key(|p| (std::cmp::Reverse(shared_folders(p, folders)), p.components().count(), p.clone()));
        found.into_iter().next()
    }
}

pub fn is_mesh_name(name: &str) -> bool {
    Path::new(name).extension().and_then(|e| e.to_str()).is_some_and(|e| MESH_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

/// The lower-cased components of a recorded path, file name last.
fn path_parts(recorded: &str) -> Vec<String> {
    recorded.split(['/', '\\']).filter(|p| !p.is_empty()).map(str::to_lowercase).collect()
}

/// How many of `path`'s parent folders, from the nearest up, equal the recorded `folders` from the last one back.
fn shared_folders(path: &Path, folders: &[String]) -> usize {
    let parents = path.parent().into_iter().flat_map(|p| p.components().rev()).map(|c| c.as_os_str().to_string_lossy().to_lowercase());
    parents.zip(folders.iter().rev()).take_while(|(a, b)| a == *b).count()
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
        let root = std::env::temp_dir().join(format!("mc-bake-finder-{}", std::process::id()));
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
        // a recorded path picks the file in the matching folder over a nearer one
        assert_eq!(finder.find("D:\\x\\sub\\deeper\\Vase HP.obj"), Some(project.join("sub/deeper/Vase HP.obj")));
        assert_eq!(finder.find("C:/elsewhere/VASE HP.OBJ"), Some(project.join("sub/vase hp.OBJ")));
        assert_eq!(finder.find("D:/x/"), None);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
