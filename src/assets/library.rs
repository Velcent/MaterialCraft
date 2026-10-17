//! The asset library the editor holds: the catalogue and the watcher, wired together. The editor
//! passes in the roots from its settings and calls [`Library::poll`] on a timer, off the UI thread.

use super::{Asset, AssetChange, Catalogue, LibraryRoot, RootId, Watcher};
use crate::diag::Sink;

/// What one [`Library::poll`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Refresh {
    /// Assets that appeared, changed or disappeared.
    pub assets: Vec<AssetChange>,
    /// Roots whose folder tree changed.
    pub folders: Vec<RootId>,
}

impl Refresh {
    pub fn is_empty(&self) -> bool {
        self.assets.is_empty() && self.folders.is_empty()
    }
}

/// Catalogue plus watcher.
#[derive(Default)]
pub struct Library {
    catalogue: Catalogue,
    watcher: Watcher,
}

impl Library {
    pub fn new() -> Library {
        Library::default()
    }

    /// Library over `roots`, each scanned once.
    pub fn with_roots(roots: impl IntoIterator<Item = LibraryRoot>, sink: &dyn Sink) -> Library {
        let mut lib = Library::new();
        for r in roots {
            lib.add_root(r, sink);
        }
        lib
    }

    /// Add (or replace) a root. A folder-backed root is scanned and watched; an in-memory root starts
    /// empty until [`Library::set_assets`].
    pub fn add_root(&mut self, root: LibraryRoot, sink: &dyn Sink) {
        let id = root.id.clone();
        let dir = root.path.clone();
        self.catalogue.add_root(root);
        match dir {
            Some(dir) => {
                let listing = self.watcher.watch(id.clone(), dir, sink);
                self.catalogue.load(&id, listing, sink);
            }
            None => {
                self.watcher.unwatch(&id);
                self.catalogue.set_assets(&id, []);
            }
        }
    }

    /// Remove a root and stop watching it.
    pub fn remove_root(&mut self, id: &RootId) -> bool {
        self.watcher.unwatch(id);
        self.catalogue.remove_root(id)
    }

    /// Make the roots exactly `roots`, in that order: new roots and roots whose folder changed are
    /// scanned, removed ones dropped, the others keep their assets.
    pub fn set_roots(&mut self, roots: Vec<LibraryRoot>, sink: &dyn Sink) {
        let stale: Vec<RootId> = self.catalogue.roots().filter(|r| !roots.iter().any(|n| n.id == r.id)).map(|r| r.id.clone()).collect();
        for id in &stale {
            self.remove_root(id);
        }
        let order: Vec<RootId> = roots.iter().map(|r| r.id.clone()).collect();
        for r in roots {
            if self.catalogue.root(&r.id).is_some_and(|old| old.path == r.path) {
                self.catalogue.add_root(r);
            } else {
                self.add_root(r, sink);
            }
        }
        self.catalogue.reorder(&order);
    }

    /// Poll every watched folder and bring the catalogue up to date.
    pub fn poll(&mut self, sink: &dyn Sink) -> Refresh {
        let mut out = Refresh::default();
        for rc in self.watcher.poll(sink) {
            if let Some(listing) = self.watcher.listing(&rc.root) {
                out.assets.extend(self.catalogue.update(&rc.root, listing, &rc.changes, sink));
            }
            if rc.folders_changed {
                out.folders.push(rc.root);
            }
        }
        out
    }

    pub fn catalogue(&self) -> &Catalogue {
        &self.catalogue
    }

    /// Replace the assets of an in-memory root (session, project; AS-21).
    pub fn set_assets(&mut self, id: &RootId, assets: impl IntoIterator<Item = Asset>) {
        self.catalogue.set_assets(id, assets);
    }
}
