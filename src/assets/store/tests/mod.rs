//! Tests of the asset store: each operation ([`ops`]), refusals ([`refusals`]), every failing step rolled back
//! ([`failures`]), the catalogue updated at once ([`catalogue`]) and documents opening after their assets were
//! renamed or deleted ([`documents`]).

mod catalogue;
mod documents;
mod failures;
mod ops;
mod refusals;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::assets::{Draft, LibraryRoot, Provenance, RootKind, Sidecar, Usage};

/// A fresh, empty folder for one test.
pub(super) fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-asset-store-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn user_root(dir: &Path) -> LibraryRoot {
    LibraryRoot::folder("user", "My assets", RootKind::User, dir)
}

pub(super) fn extra_root(dir: &Path) -> LibraryRoot {
    LibraryRoot::folder("extra", "Extra", RootKind::Extra, dir)
}

pub(super) fn sidecar(name: &str) -> Sidecar {
    Sidecar {
        name: Some(name.into()),
        usage: Some(Usage::Alpha),
        tags: vec!["old".into()],
        description: Some("A scratch".into()),
        provenance: Provenance::HandMade { author: None, notes: None },
        ..Sidecar::default()
    }
}

/// An asset with all three files.
pub(super) fn full<'a>(side: &'a Sidecar) -> Draft<'a> {
    Draft { bytes: b"asset bytes", sidecar: Some(side), thumbnail: Some(b"thumbnail bytes") }
}

/// Every file under `dir` (hidden ones too) with its bytes, and every folder (as `name/`).
pub(super) fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if e.file_type().unwrap().is_dir() {
                out.insert(format!("{rel}/"), Vec::new());
                walk(&e.path(), &rel, out);
            } else {
                out.insert(rel, fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, "", &mut out);
    out
}

/// The names in a snapshot.
pub(super) fn names(s: &BTreeMap<String, Vec<u8>>) -> Vec<&str> {
    s.keys().map(String::as_str).collect()
}
