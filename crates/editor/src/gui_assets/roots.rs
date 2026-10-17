//! Which libraries the tray shows (AS-01, AS-18): the user's own library in `library/` next to the settings
//! file, created with the standard folder layout on first run (AS-02), and the further folders added in the
//! library settings. The starter library joins here once it ships (roadmap step 25).

use std::path::{Path, PathBuf};

use materialcraft::assets::{LibraryRoot, RootKind, create_standard_layout};

/// The id of the user's own library.
pub const USER_ROOT: &str = "user";

/// Folder of the user's library under the settings folder.
pub const USER_FOLDER: &str = "library";

/// The id of a further library: its folder, so saved searches keep pointing at it when others are removed.
pub fn extra_id(path: &Path) -> String {
    format!("extra:{}", path.display())
}

/// The user's library folder for settings kept in `settings_dir` (none when settings live in memory).
pub fn user_folder(settings_dir: Option<&Path>) -> Option<PathBuf> {
    settings_dir.map(|d| d.join(USER_FOLDER))
}

/// The roots for the user library at `user` and the further folders `extra`, in tray order.
pub fn roots(user: Option<&Path>, extra: &[PathBuf]) -> Vec<LibraryRoot> {
    let mut out = Vec::new();
    if let Some(u) = user {
        out.push(LibraryRoot::folder(USER_ROOT, "My assets", RootKind::User, u));
    }
    for p in extra {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
        let id = extra_id(p);
        if !out.iter().any(|r: &LibraryRoot| r.id.as_str() == id) {
            out.push(LibraryRoot::folder(id, name, RootKind::Extra, p.clone()));
        }
    }
    out
}

/// Creates the user library with its standard layout if its folder does not exist yet (first run). Returns
/// what to tell the user, if anything.
pub fn first_run(user: &Path) -> Option<String> {
    if user.exists() {
        return None;
    }
    let root = LibraryRoot::folder(USER_ROOT, "My assets", RootKind::User, user);
    match create_standard_layout(&root) {
        Ok(_) => Some(format!("Created your asset library in {}.", user.display())),
        Err(e) => Some(format!("Your asset library could not be created in {}: {e}.", user.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_library_comes_first_and_extras_are_unique() {
        let r = roots(Some(Path::new("/s/library")), &[PathBuf::from("/a/x"), PathBuf::from("/a/x"), PathBuf::from("/b")]);
        let ids: Vec<&str> = r.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["user", "extra:/a/x", "extra:/b"]);
        assert_eq!(r[1].name, "x");
        assert!(roots(None, &[]).is_empty());
    }

    #[test]
    fn first_run_creates_the_standard_layout_once() {
        let dir = std::env::temp_dir().join(format!("mc-tray-first-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let user = dir.join(USER_FOLDER);
        assert!(first_run(&user).is_some_and(|m| m.starts_with("Created")));
        assert!(user.join("materials").is_dir() && user.join("smart-masks").is_dir());
        assert_eq!(first_run(&user), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
