//! The catalogue: every asset of every library root, kept in the editor's root order, and the
//! search over them. It is built from [`Listing`]s and updated from their differences; it never
//! touches the file system itself except to read sidecars through [`AssetBuilder`].

use std::collections::{BTreeMap, BTreeSet};

use super::build::AssetBuilder;
use super::format::FileRole;
use super::query::RootKeys;
use super::{Asset, ChangeKind, FileChange, LibraryRoot, Listing, RootId, Search};
use crate::diag::{Area, Level, Sink, SinkExt, Target};

/// One asset found by a search, with its library.
#[derive(Debug, Clone, Copy)]
pub struct Hit<'a> {
    pub root: &'a LibraryRoot,
    pub asset: &'a Asset,
}

/// An asset that appeared, changed or disappeared in the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetChange {
    pub root: RootId,
    pub path: String,
    pub kind: ChangeKind,
}

struct RootEntry {
    root: LibraryRoot,
    assets: BTreeMap<String, Asset>,
    folders: BTreeSet<String>,
}

/// All assets of all roots.
#[derive(Default)]
pub struct Catalogue {
    roots: Vec<RootEntry>,
}

impl Catalogue {
    pub fn new() -> Catalogue {
        Catalogue::default()
    }

    /// Add a root at the end, or replace the root with the same id in place (keeping its assets
    /// until it is loaded again).
    pub fn add_root(&mut self, root: LibraryRoot) {
        match self.entry_mut(&root.id) {
            Some(e) => e.root = root,
            None => self.roots.push(RootEntry { root, assets: BTreeMap::new(), folders: BTreeSet::new() }),
        }
    }

    /// Remove a root and its assets. Returns false if it wasn't there.
    pub fn remove_root(&mut self, id: &RootId) -> bool {
        let before = self.roots.len();
        self.roots.retain(|e| &e.root.id != id);
        self.roots.len() != before
    }

    /// Put the roots in the order of `ids`; roots not named keep their relative order at the end.
    pub fn reorder(&mut self, ids: &[RootId]) {
        self.roots.sort_by_key(|e| ids.iter().position(|id| id == &e.root.id).unwrap_or(usize::MAX));
    }

    /// Roots in order.
    pub fn roots(&self) -> impl Iterator<Item = &LibraryRoot> {
        self.roots.iter().map(|e| &e.root)
    }

    pub fn root(&self, id: &RootId) -> Option<&LibraryRoot> {
        self.entry(id).map(|e| &e.root)
    }

    /// Rebuild a folder-backed root from a full listing of its folder.
    pub fn load(&mut self, id: &RootId, listing: &Listing, sink: &dyn Sink) {
        let Some(entry) = self.entry_mut(id) else { return };
        let Some(dir) = entry.root.path.clone() else { return };
        let builder = AssetBuilder { root_dir: &dir, listing, sink };
        entry.assets.clear();
        for (path, _) in listing.files() {
            match FileRole::of(path) {
                FileRole::Asset(format) => {
                    entry.assets.insert(path.to_owned(), builder.build(path, format));
                }
                FileRole::Sidecar { asset } => report_orphan_sidecar(listing, path, asset, sink),
                FileRole::Thumbnail { .. } | FileRole::Other => {}
            }
        }
        entry.folders = listing.folders().clone();
    }

    /// Apply file changes of a folder-backed root; `listing` is the root's listing *after* them.
    /// Returns the assets that were added, changed or removed.
    pub fn update(&mut self, id: &RootId, listing: &Listing, changes: &[FileChange], sink: &dyn Sink) -> Vec<AssetChange> {
        let Some(entry) = self.entry_mut(id) else { return Vec::new() };
        let Some(dir) = entry.root.path.clone() else { return Vec::new() };
        let builder = AssetBuilder { root_dir: &dir, listing, sink };
        let mut touched = BTreeSet::new();
        for change in changes {
            match FileRole::of(&change.path) {
                FileRole::Asset(_) => {
                    touched.insert(change.path.as_str());
                }
                FileRole::Sidecar { asset } => {
                    if change.kind != ChangeKind::Removed {
                        report_orphan_sidecar(listing, &change.path, asset, sink);
                    }
                    touched.insert(asset);
                }
                FileRole::Thumbnail { asset } => {
                    touched.insert(asset);
                }
                FileRole::Other => {}
            }
        }
        let mut out = Vec::new();
        for path in touched {
            let FileRole::Asset(format) = FileRole::of(path) else { continue };
            let kind = if listing.contains(path) {
                let asset = builder.build(path, format);
                match entry.assets.insert(path.to_owned(), asset) {
                    None => Some(ChangeKind::Added),
                    Some(_) => Some(ChangeKind::Modified),
                }
            } else {
                entry.assets.remove(path).map(|_| ChangeKind::Removed)
            };
            if let Some(kind) = kind {
                out.push(AssetChange { root: id.clone(), path: path.to_owned(), kind });
            }
        }
        entry.folders = listing.folders().clone();
        out
    }

    /// Replace the assets of a root (for in-memory roots: session and project scopes, AS-21).
    pub fn set_assets(&mut self, id: &RootId, assets: impl IntoIterator<Item = Asset>) {
        let Some(entry) = self.entry_mut(id) else { return };
        entry.assets.clear();
        entry.folders.clear();
        for mut a in assets {
            a.reindex();
            let mut folder = a.folder();
            while !folder.is_empty() {
                entry.folders.insert(folder.to_owned());
                folder = super::format::folder_of(folder);
            }
            entry.assets.insert(a.path.clone(), a);
        }
    }

    pub fn get(&self, id: &RootId, path: &str) -> Option<&Asset> {
        self.entry(id)?.assets.get(path)
    }

    /// The assets of a root, in path order.
    pub fn assets(&self, id: &RootId) -> impl Iterator<Item = &Asset> {
        self.entry(id).into_iter().flat_map(|e| e.assets.values())
    }

    /// The folders of a root, in path order, including empty ones (AS-11, AS-13).
    pub fn folders(&self, id: &RootId) -> impl Iterator<Item = &str> {
        self.entry(id).into_iter().flat_map(|e| e.folders.iter().map(String::as_str))
    }

    /// Number of assets over all roots.
    pub fn len(&self) -> usize {
        self.roots.iter().map(|e| e.assets.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every asset matching `search`, in root order, then path order.
    pub fn find(&self, search: &Search) -> Vec<Hit<'_>> {
        let mut hits = Vec::new();
        for e in &self.roots {
            if search.location.as_ref().is_some_and(|l| l.root != e.root.id) {
                continue;
            }
            let keys = RootKeys::of(&e.root);
            for a in e.assets.values() {
                if search.allows_usage(a.usage)
                    && !(search.hide_importable && a.is_importable_only())
                    && search.location.as_ref().is_none_or(|l| l.contains(&e.root.id, a.folder()))
                    && search.query.matches(a, &keys)
                {
                    hits.push(Hit { root: &e.root, asset: a });
                }
            }
        }
        hits
    }

    fn entry(&self, id: &RootId) -> Option<&RootEntry> {
        self.roots.iter().find(|e| &e.root.id == id)
    }

    fn entry_mut(&mut self, id: &RootId) -> Option<&mut RootEntry> {
        self.roots.iter_mut().find(|e| &e.root.id == id)
    }
}

fn report_orphan_sidecar(listing: &Listing, sidecar: &str, asset: &str, sink: &dyn Sink) {
    let has_asset = listing.contains(asset) && matches!(FileRole::of(asset), FileRole::Asset(_));
    if !has_asset {
        sink.about(
            Level::Warning,
            Area::Assets,
            Target::Asset(sidecar.to_owned()),
            format!("sidecar without an asset: {asset} is missing or not an asset file"),
        );
    }
}
