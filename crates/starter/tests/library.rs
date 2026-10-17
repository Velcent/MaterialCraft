//! The starter library's gate (roadmap step 25): building it twice gives the same bytes, every asset has a
//! provenance sidecar and a thumbnail, and the asset library finds them all as a read-only starter root.

use materialcraft::assets::{Library, LibraryRoot, Provenance, RootId, RootKind, Search, Thumbnail, Usage};
use materialcraft::diag::{Entry, Sink};
use materialcraft_starter::{LIBRARY_VERSION, Options, build, installed_version, is_current};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
struct Log(Mutex<Vec<String>>);

impl Sink for Log {
    fn emit(&self, entry: Entry) {
        self.0.lock().unwrap().push(entry.text);
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-starter-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Every file under `dir`, by relative path.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

#[test]
fn the_starter_library_is_deterministic_complete_and_found_by_the_tray() {
    let root = scratch("gate");
    let (a, b) = (root.join("a"), root.join("b"));
    let small = Options { texture_size: 64, jobs: 4 };
    let built = build(&a, &small).unwrap();
    build(&b, &Options { jobs: 1, ..small.clone() }).unwrap();
    let (fa, fb) = (files(&a), files(&b));
    assert_eq!(fa.keys().collect::<Vec<_>>(), fb.keys().collect::<Vec<_>>());
    for (path, bytes) in &fa {
        assert!(fb.get(path) == Some(bytes), "{path} differs between two builds");
    }

    // Rebuilding over an existing starter library replaces it with the same bytes.
    build(&a, &small).unwrap();
    assert_eq!(files(&a), fa);
    assert!(is_current(&a));
    assert_eq!(installed_version(&a), Some(LIBRARY_VERSION));

    // Counts from ADR 0008 rm-starter-scope.
    let count = |u: Usage| built.assets.iter().filter(|(_, x)| *x == u).count();
    assert!(count(Usage::Material) >= 30, "{}", count(Usage::Material));
    assert!(count(Usage::SmartMask) >= 10);
    assert!(count(Usage::Texture) >= 20);
    assert!(count(Usage::Alpha) >= 8);
    assert_eq!(count(Usage::Environment), 1);

    let log = Log::default();
    let starter = LibraryRoot::folder("starter", "Starter", RootKind::Starter, &a);
    assert!(starter.is_read_only());
    let lib = Library::with_roots([starter], &log);
    assert!(log.0.lock().unwrap().is_empty(), "{:?}", log.0.lock().unwrap());
    let assets: Vec<_> = lib.catalogue().assets(&RootId::new("starter")).collect();
    assert_eq!(assets.len(), built.assets.len());
    for (path, usage) in &built.assets {
        let asset = lib.catalogue().get(&RootId::new("starter"), path).unwrap_or_else(|| panic!("{path} not in the catalogue"));
        assert!(asset.has_sidecar, "{path} has no sidecar");
        assert_eq!(asset.usage, *usage, "{path}");
        assert!(!matches!(asset.provenance, Provenance::Unknown), "{path} records no provenance");
        assert!(!asset.tags.is_empty() && asset.description.is_some(), "{path}");
        let thumb_ok = match &asset.thumbnail {
            Thumbnail::File(t) => a.join(t).is_file(),
            Thumbnail::Embedded => true,
            Thumbnail::None | Thumbnail::OwnImage => false,
        };
        assert!(thumb_ok, "{path} has no thumbnail: {:?}", asset.thumbnail);
    }
    assert!(lib.catalogue().assets(&RootId::new("starter")).filter(|a| a.usage == Usage::SmartMask).all(|a| !a.needs.is_empty()));
    assert!(!lib.catalogue().find(&Search::text("rust")).is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_folder_that_is_not_a_starter_library_is_never_replaced() {
    let root = scratch("refuse");
    std::fs::write(root.join("mine.txt"), "keep").unwrap();
    assert!(build(&root, &Options { texture_size: 16, jobs: 2 }).is_err());
    assert!(root.join("mine.txt").is_file());
    let _ = std::fs::remove_dir_all(&root);
}
