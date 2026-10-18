//! Where a new asset saved into a library goes: its usage's folder of the standard layout (AS-02) and a file name
//! made from its display name that does not replace an existing file.

use std::path::{Path, PathBuf};

use super::Usage;

/// Longest file stem made from a name.
const MAX_STEM: usize = 64;

/// `<root>/<usage folder>/<name>.<extension>`, the name made safe as a file name (letters, digits, spaces, `-`, `_`
/// and `.` kept, anything else `_`; never empty or hidden) and numbered `name 2`, `name 3`, … while the file exists.
pub fn new_asset_path(root: &Path, usage: Usage, name: &str, extension: &str) -> PathBuf {
    let rel = new_asset_name(usage, name, extension, |rel| root.join(rel).exists());
    root.join(rel)
}

/// The library-relative path (`<usage folder>/<name>.<extension>`) [`new_asset_path`] picks, numbered while `taken`
/// says a candidate is in use.
pub(crate) fn new_asset_name(usage: Usage, name: &str, extension: &str, taken: impl Fn(&str) -> bool) -> String {
    let stem = file_stem(name, usage);
    let mut n = 1u32;
    loop {
        let candidate = if n == 1 { format!("{}/{stem}.{extension}", usage.folder()) } else { format!("{}/{stem} {n}.{extension}", usage.folder()) };
        if !taken(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

fn file_stem(name: &str, usage: Usage) -> String {
    safe_stem(name).unwrap_or_else(|| usage.keyword().to_string())
}

/// `name` made safe as a file stem, or `None` when nothing of it is left.
pub(crate) fn safe_stem(name: &str) -> Option<String> {
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).take(MAX_STEM).collect();
    let safe = safe.trim().trim_start_matches('.').trim().to_string();
    (!safe.is_empty()).then_some(safe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_safe_and_never_replace_a_file() {
        let dir = std::env::temp_dir().join(format!("materialcraft-naming-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p = new_asset_path(&dir, Usage::SmartMask, "Worn: edges/2", "mcfrag");
        assert_eq!(p, dir.join("smart-masks").join("Worn_ edges_2.mcfrag"));
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, b"x").unwrap();
        assert_eq!(new_asset_path(&dir, Usage::SmartMask, "Worn: edges/2", "mcfrag").file_name().unwrap(), "Worn_ edges_2 2.mcfrag");
        assert_eq!(new_asset_path(&dir, Usage::SmartMaterial, " ..", "mcfrag").file_name().unwrap(), "smart-material.mcfrag");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
