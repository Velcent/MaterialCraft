//! A snapshot of the files under a library folder, and the difference between two snapshots. The
//! catalogue is built from a listing; the watcher compares listings poll by poll.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::diag::{Area, Level, Sink, SinkExt, Target};

/// Folders nested deeper than this are not entered (guards against pathological trees).
pub const MAX_DEPTH: usize = 32;

/// What identifies one version of a file: change either and the file counts as modified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

/// Every file under a folder, by root-relative `/`-separated path, plus every folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listing {
    files: BTreeMap<String, Stamp>,
    folders: BTreeSet<String>,
}

/// How a file changed between two listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeKind {
    Added,
    Modified,
    Removed,
}

/// One changed file, root-relative.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileChange {
    pub path: String,
    pub kind: ChangeKind,
}

impl Listing {
    /// A listing with no files.
    pub const fn empty() -> Listing {
        Listing { files: BTreeMap::new(), folders: BTreeSet::new() }
    }

    /// List `dir` recursively. Hidden entries (starting with `.`) and symbolic links to folders are
    /// skipped; folders that can't be read are reported to `sink` and skipped. A missing `dir` gives an
    /// empty listing and one warning.
    pub fn read(dir: &Path, sink: &dyn Sink) -> Listing {
        let mut listing = Listing::default();
        if !dir.is_dir() {
            sink.about(Level::Warning, Area::Assets, Target::File(dir.display().to_string()), "library folder not found");
            return listing;
        }
        listing.walk(dir, "", 0, sink);
        listing
    }

    fn walk(&mut self, dir: &Path, prefix: &str, depth: usize, sink: &dyn Sink) {
        if depth > MAX_DEPTH {
            sink.about(
                Level::Warning,
                Area::Assets,
                Target::File(dir.display().to_string()),
                format!("folder nested deeper than {MAX_DEPTH} levels; not scanned"),
            );
            return;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => {
                sink.about(Level::Warning, Area::Assets, Target::File(dir.display().to_string()), format!("can't read folder: {e}"));
                return;
            }
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                sink.about(Level::Warning, Area::Assets, Target::File(entry.path().display().to_string()), "file name is not valid Unicode; skipped");
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            let Ok(file_type) = entry.file_type() else { continue };
            if file_type.is_dir() {
                self.folders.insert(rel.clone());
                self.walk(&entry.path(), &rel, depth + 1, sink);
            } else if let Ok(meta) = fs::metadata(entry.path())
                && meta.is_file()
            {
                // Symbolic links to files are followed; links to folders are not (no cycles).
                self.files.insert(rel, Stamp { modified: meta.modified().ok(), len: meta.len() });
            }
        }
    }

    /// Stamp of a file, if listed.
    pub fn get(&self, path: &str) -> Option<&Stamp> {
        self.files.get(path)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(path)
    }

    /// Files in path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &Stamp)> {
        self.files.iter().map(|(p, s)| (p.as_str(), s))
    }

    /// Folders, root-relative, in path order.
    pub fn folders(&self) -> &BTreeSet<String> {
        &self.folders
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// What changed from `self` (older) to `newer`, in path order.
    pub fn diff(&self, newer: &Listing) -> Vec<FileChange> {
        let mut out = Vec::new();
        for (path, stamp) in &newer.files {
            match self.files.get(path) {
                None => out.push(FileChange { path: path.clone(), kind: ChangeKind::Added }),
                Some(old) if old != stamp => out.push(FileChange { path: path.clone(), kind: ChangeKind::Modified }),
                Some(_) => {}
            }
        }
        for path in self.files.keys() {
            if !newer.files.contains_key(path) {
                out.push(FileChange { path: path.clone(), kind: ChangeKind::Removed });
            }
        }
        out.sort();
        out
    }
}
