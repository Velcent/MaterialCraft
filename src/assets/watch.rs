//! Folder watching by polling (AS-19), std only: each poll lists every watched folder again and
//! compares it with the previous listing. A change made before a poll is reported by that poll.
//!
//! Problems found while listing (an unreadable folder, a missing library) are reported once, when
//! they first appear, not on every poll.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use super::{FileChange, Listing, RootId};
use crate::diag::{Entry, Sink};

/// What changed under one root since the previous poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootChanges {
    pub root: RootId,
    pub changes: Vec<FileChange>,
    /// True if folders appeared or disappeared (the tray's folder tree changes).
    pub folders_changed: bool,
}

/// Polls library folders for changes.
#[derive(Default)]
pub struct Watcher {
    watched: Vec<Watched>,
}

static EMPTY: Listing = Listing::empty();

impl Watcher {
    pub fn new() -> Watcher {
        Watcher::default()
    }

    /// Watch `dir` for `root`: list it now (reporting problems to `sink`) and return the listing to
    /// load the catalogue from, so the first poll reports only what changed since. Replaces an
    /// earlier watch of the same root.
    pub fn watch(&mut self, root: RootId, dir: PathBuf, sink: &dyn Sink) -> &Listing {
        self.unwatch(&root);
        let mut w = Watched { root, dir, listing: Listing::empty(), reported: HashSet::new() };
        w.listing = w.list(sink);
        let i = self.watched.len();
        self.watched.push(w);
        self.watched.get(i).map_or(&EMPTY, |w| &w.listing)
    }

    /// Stop watching a root. Returns false if it wasn't watched.
    pub fn unwatch(&mut self, root: &RootId) -> bool {
        let before = self.watched.len();
        self.watched.retain(|w| &w.root != root);
        self.watched.len() != before
    }

    /// The current listing of a watched root.
    pub fn listing(&self, root: &RootId) -> Option<&Listing> {
        self.watched.iter().find(|w| &w.root == root).map(|w| &w.listing)
    }

    /// Looks at the files `paths` of `root` again (after the asset store wrote them) and returns what changed, so the
    /// next poll doesn't report it a second time. `None` if the root isn't watched or nothing changed.
    pub fn restat(&mut self, root: &RootId, paths: &[String]) -> Option<RootChanges> {
        let w = self.watched.iter_mut().find(|w| &w.root == root)?;
        let folders = w.listing.folders().len();
        let changes = w.listing.restat(&w.dir, paths);
        let folders_changed = w.listing.folders().len() != folders;
        (!changes.is_empty() || folders_changed).then(|| RootChanges { root: root.clone(), changes, folders_changed })
    }

    /// List every watched folder again and return what changed, per root (roots where nothing
    /// changed are left out).
    pub fn poll(&mut self, sink: &dyn Sink) -> Vec<RootChanges> {
        let mut out = Vec::new();
        for w in &mut self.watched {
            let listing = w.list(sink);
            let changes = w.listing.diff(&listing);
            let folders_changed = w.listing.folders() != listing.folders();
            w.listing = listing;
            if !changes.is_empty() || folders_changed {
                out.push(RootChanges { root: w.root.clone(), changes, folders_changed });
            }
        }
        out
    }
}

struct Watched {
    root: RootId,
    dir: PathBuf,
    listing: Listing,
    /// Problems reported by the previous listing, by [`problem_key`].
    reported: HashSet<String>,
}

impl Watched {
    /// List the folder, passing on only problems the previous listing didn't have.
    fn list(&mut self, sink: &dyn Sink) -> Listing {
        let problems = Gather::default();
        let listing = Listing::read(&self.dir, &problems);
        let mut now = HashSet::new();
        for entry in problems.0.into_inner().unwrap_or_else(|e| e.into_inner()) {
            let key = problem_key(&entry);
            if !self.reported.contains(&key) {
                sink.emit(entry);
            }
            now.insert(key);
        }
        self.reported = now;
        listing
    }
}

/// An entry without its time, so the same problem on two polls compares equal.
fn problem_key(e: &Entry) -> String {
    match &e.target {
        Some(t) => format!("{} {t} {}", e.level.name(), e.text),
        None => format!("{} {}", e.level.name(), e.text),
    }
}

/// Collects one listing's problems so repeats can be filtered.
#[derive(Default)]
struct Gather(Mutex<Vec<Entry>>);

impl Sink for Gather {
    fn emit(&self, entry: Entry) {
        if let Ok(mut v) = self.0.lock() {
            v.push(entry);
        }
    }
}
