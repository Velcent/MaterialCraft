//! [`HighPolyRequest`]: which mesh Painter baked a set from, as its settings record it.

use super::painter::MaterialBaking;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HighPolyRequest {
    /// "Use low poly mesh as high poly mesh".
    pub low_as_high: bool,
    /// File names of the high-poly meshes (only the last path component; the paths are from the machine
    /// Painter ran on, e.g. `file:///D:/…/Sload HP.OBJ`).
    pub files: Vec<String>,
}

impl HighPolyRequest {
    pub fn of(baking: &MaterialBaking) -> HighPolyRequest {
        HighPolyRequest {
            low_as_high: baking.detail.bool("Detail.LowAsHigh") == Some(true),
            files: baking
                .detail
                .text("Detail.HipolyMesh")
                .map(file_names)
                .unwrap_or_default(),
        }
    }
}

/// File names in Painter's list of URLs (`file:///a/b.obj`, several separated by `|`, Observed on
/// `AMULET 5.spp`).
fn file_names(urls: &str) -> Vec<String> {
    urls.split(['|', ';'])
        .map(|u| u.trim().trim_start_matches("file:"))
        .map(|u| u.trim().trim_end_matches(['/', '\\']))
        .filter_map(|u| u.rsplit(['/', '\\']).next())
        .map(|n| n.replace("%20", " "))
        .filter(|n| !n.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_file_names() {
        assert_eq!(
            file_names("file:///D:/Users/x/Thras/The Sload/Sload HP.OBJ"),
            ["Sload HP.OBJ"]
        );
        assert_eq!(
            file_names("file:///C:/a.fbx|file:///C:/b%20c.obj"),
            ["a.fbx", "b c.obj"]
        );
        assert!(file_names("").is_empty());
    }
}
