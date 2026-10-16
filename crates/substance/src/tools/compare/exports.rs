//! [`ExportFinder`]: Painter's exported texture files for a texture set, recognised by name:
//! `[<prefix>_]<set>_<map>.<ext>`, e.g. `Gondola_default_BaseColor.png` or `LPMOOO_Seil_2_Roughness.png`.
//! Packed maps (`OcclusionRoughnessMetallic` / `ORM`) yield one entry per component.

use materialcraft::paint::Channel;
use std::path::{Path, PathBuf};

/// How deep below the export folder to look.
const MAX_DEPTH: usize = 3;

/// Which values of an exported file to compare with which channel.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportMap {
    pub channel: Channel,
    pub path: PathBuf,
    /// Component of a packed map (0 = R …); `None` = the whole colour or gray value.
    pub component: Option<usize>,
    /// The exported normal uses DirectX's convention (green flipped).
    pub directx: bool,
    pub suffix: String,
}

/// Map-name suffixes Painter's presets use, lower case, with the channel and packed component.
const SUFFIXES: &[(&str, Channel, Option<usize>, bool)] = &[
    ("basecolor", Channel::BaseColor, None, false),
    ("base_color", Channel::BaseColor, None, false),
    ("roughness", Channel::Roughness, None, false),
    ("metallic", Channel::Metallic, None, false),
    ("metalness", Channel::Metallic, None, false),
    ("normal", Channel::Normal, None, false),
    ("normal_opengl", Channel::Normal, None, false),
    ("normal_directx", Channel::Normal, None, true),
    ("height", Channel::Height, None, false),
    ("mixed_ao", Channel::AmbientOcclusion, None, false),
    ("ambientocclusion", Channel::AmbientOcclusion, None, false),
    ("ao", Channel::AmbientOcclusion, None, false),
    ("emissive", Channel::Emissive, None, false),
    ("opacity", Channel::Opacity, None, false),
];
const PACKED: &[&str] = &["occlusionroughnessmetallic", "orm"];
const PACKED_CHANNELS: [Channel; 3] = [Channel::AmbientOcclusion, Channel::Roughness, Channel::Metallic];

pub struct ExportFinder {
    files: Vec<PathBuf>,
}

impl ExportFinder {
    /// Lists image files below `dir`.
    pub fn scan(dir: &Path) -> ExportFinder {
        let mut files = Vec::new();
        collect(dir, 0, &mut files);
        files.sort();
        ExportFinder { files }
    }

    /// Exports of the set `set`; when several set names match a file, the longest wins (`all_sets`).
    pub fn for_set(&self, set: &str, all_sets: &[String]) -> Vec<ExportMap> {
        let mut out = Vec::new();
        for path in &self.files {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Some(suffix) = suffix_after_set(stem, set) else {
                continue;
            };
            let longer_match = all_sets
                .iter()
                .any(|other| other.len() > set.len() && suffix_after_set(stem, other).is_some());
            if longer_match {
                continue;
            }
            out.extend(maps_for(path, &suffix));
        }
        out
    }
}

/// The map name after `<set>_` if the stem is `<set>_<map>` or `…_<set>_<map>`.
fn suffix_after_set(stem: &str, set: &str) -> Option<String> {
    let needle = format!("{set}_");
    let at = stem.rfind(&needle)?;
    if at > 0 && !stem[..at].ends_with('_') {
        return None;
    }
    Some(stem[at + needle.len()..].to_ascii_lowercase())
}

fn maps_for(path: &Path, suffix: &str) -> Vec<ExportMap> {
    let map = |channel, component, directx| ExportMap {
        channel,
        path: path.to_path_buf(),
        component,
        directx,
        suffix: suffix.to_string(),
    };
    if PACKED.contains(&suffix) {
        return PACKED_CHANNELS
            .iter()
            .enumerate()
            .map(|(i, c)| map(*c, Some(i), false))
            .collect();
    }
    SUFFIXES
        .iter()
        .find(|(s, ..)| *s == suffix)
        .map(|(_, c, comp, dx)| vec![map(*c, *comp, *dx)])
        .unwrap_or_default()
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth < MAX_DEPTH {
                collect(&path, depth + 1, out);
            }
        } else if is_image(&path) {
            out.push(path);
        }
    }
}

fn is_image(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    matches!(
        ext.as_deref(),
        Some("png" | "tga" | "jpg" | "jpeg" | "tif" | "tiff" | "exr")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_set_names_with_prefixes_and_packing() {
        assert_eq!(
            suffix_after_set("LPMOOO_Seil_2_BaseColor", "Seil_2").as_deref(),
            Some("basecolor")
        );
        assert_eq!(
            suffix_after_set("LPMOOO_Seil_2_BaseColor", "Seil").as_deref(),
            Some("2_basecolor")
        );
        assert_eq!(
            suffix_after_set("Default_BaseColor", "Default").as_deref(),
            Some("basecolor")
        );
        assert_eq!(suffix_after_set("XDefault_BaseColor", "Default"), None);
        let packed = maps_for(Path::new("a_ORM.png"), "orm");
        assert_eq!(packed.len(), 3);
        assert_eq!((packed[1].channel, packed[1].component), (Channel::Roughness, Some(1)));
        assert!(maps_for(Path::new("x.png"), "normal_directx")[0].directx);
        assert!(maps_for(Path::new("x.png"), "unknown").is_empty());
    }
}
