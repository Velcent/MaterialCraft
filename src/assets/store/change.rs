//! [`StoreChange`]: what one store operation did, so the library can update its catalogue at once
//! ([`Library::apply`](crate::assets::Library::apply)) instead of waiting for the watcher's next poll.

use super::bundle::relative_files;
use crate::assets::RootId;

/// An asset in a library: its root and root-relative path.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetRef {
    pub root: RootId,
    pub path: String,
}

impl AssetRef {
    pub fn new(root: RootId, path: impl Into<String>) -> AssetRef {
        AssetRef { root, path: path.into() }
    }
}

/// What a store operation changed on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreChange {
    /// The asset before the operation (`None` for a new asset).
    pub before: Option<AssetRef>,
    /// The asset after the operation (`None` after a delete).
    pub after: Option<AssetRef>,
    /// Hidden temporary files the store could not remove after succeeding (invisible to the library), for the log.
    pub warnings: Vec<String>,
}

impl StoreChange {
    /// Every root-relative file that may have changed (each asset's file, sidecar and thumbnail), by root.
    pub fn files(&self) -> Vec<(RootId, String)> {
        let mut out: Vec<(RootId, String)> = Vec::new();
        for a in self.before.iter().chain(&self.after) {
            for f in relative_files(&a.path) {
                let entry = (a.root.clone(), f);
                if !out.contains(&entry) {
                    out.push(entry);
                }
            }
        }
        out
    }
}
