//! [`HighPolyRequest`]: which mesh Painter baked a set from, as its settings record it.

use crate::model::baking::MaterialBaking;
use materialcraft::paint::bake_config::{HighPlacement, HighPoly};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighPolyRequest {
    /// "Use low poly mesh as high poly mesh".
    pub low_as_high: bool,
    /// The high-poly meshes as Painter recorded them, without the URL scheme (paths on the machine Painter ran
    /// on, e.g. `D:/…/Sload HP.OBJ`; [`materialcraft::bake::MeshFinder`] finds them below the project).
    pub files: Vec<String>,
}

impl HighPolyRequest {
    pub fn of(baking: &MaterialBaking) -> HighPolyRequest {
        HighPolyRequest {
            low_as_high: baking.detail.bool("Detail.LowAsHigh") == Some(true),
            files: baking
                .detail
                .text("Detail.HipolyMesh")
                .map(file_paths)
                .unwrap_or_default(),
        }
    }

    /// The document's high-poly source; `placement` moves the files into the document's space.
    pub fn to_high(&self, placement: Option<HighPlacement>) -> HighPoly {
        HighPoly {
            low_as_high: self.low_as_high,
            files: self.files.clone(),
            placement: (!self.files.is_empty()).then_some(placement).flatten(),
        }
    }
}

/// Paths in Painter's list of URLs (`file:///a/b.obj`, several separated by `|`, Observed on `AMULET 5.spp`),
/// without the scheme and the slash before a drive letter.
fn file_paths(urls: &str) -> Vec<String> {
    urls.split(['|', ';'])
        .map(|u| u.trim())
        .map(|u| u.strip_prefix("file://").unwrap_or(u))
        .map(|u| u.trim_end_matches(['/', '\\']).replace("%20", " "))
        .map(|u| match u.strip_prefix('/') {
            Some(rest) if is_drive_path(rest) => rest.to_string(),
            _ => u,
        })
        .filter(|u| u.rsplit(['/', '\\']).next().is_some_and(|n| !n.is_empty()))
        .collect()
}

/// `D:/…` or `D:\\…`.
fn is_drive_path(p: &str) -> bool {
    matches!(p.as_bytes(), [d, b':', b'/' | b'\\', ..] if d.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_file_paths() {
        assert_eq!(
            file_paths("file:///D:/Users/x/Thras/The Sload/Sload HP.OBJ"),
            ["D:/Users/x/Thras/The Sload/Sload HP.OBJ"]
        );
        assert_eq!(
            file_paths("file:///C:/a.fbx|file:///C:/b%20c.obj"),
            ["C:/a.fbx", "C:/b c.obj"]
        );
        assert_eq!(file_paths("file:///home/u/h.obj"), ["/home/u/h.obj"]);
        assert!(file_paths("").is_empty());
    }
}
