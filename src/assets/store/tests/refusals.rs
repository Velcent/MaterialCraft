//! What the store refuses: replacing anything, paths outside the library, files that are not assets, read-only and
//! in-memory libraries, unreadable sidecars. A refusal changes nothing.

use std::fs;

use super::*;
use crate::assets::{AssetStore, MetaEdit, StoreError};

#[test]
fn nothing_is_ever_replaced() {
    let dir = temp_dir("refuse-replace");
    let store = AssetStore::new();
    let root = user_root(&dir);
    let side = sidecar("A");
    store.create(&root, "alphas/a.png", &full(&side)).unwrap();
    store.create(&root, "alphas/b.png", &Draft::new(b"b")).unwrap();
    fs::write(dir.join("alphas/c.png.thumb.png"), b"stray").unwrap();
    let before = snapshot(&dir);
    let exists = |r: Result<_, StoreError>| matches!(r, Err(StoreError::Exists(_)));
    assert!(exists(store.create(&root, "alphas/a.png", &Draft::new(b"x"))));
    assert!(exists(store.create(&root, "alphas/c.png", &Draft::new(b"x"))), "a stray thumbnail holds the name");
    assert!(exists(store.rename(&root, "alphas/b.png", "a")));
    assert!(exists(store.rename(&root, "alphas/b.png", "c")));
    assert!(exists(store.move_to(&root, "alphas/b.png", &root, "alphas")), "moving onto itself");
    store.create(&root, "textures/a.png", &Draft::new(b"t")).unwrap();
    let before_move = snapshot(&dir);
    assert!(exists(store.move_to(&root, "textures/a.png", &root, "alphas")));
    assert_eq!(snapshot(&dir), before_move);
    assert_eq!(before.len() + 2, before_move.len());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn paths_outside_the_library_and_files_that_are_not_assets_are_refused() {
    let dir = temp_dir("refuse-paths");
    let store = AssetStore::new();
    let root = user_root(&dir.join("lib"));
    for bad in ["../x.png", "/abs/x.png", "a//x.png", "./x.png", "a/../x.png", "C:x.png", "a\\x.png", ".hidden.png", "a/.h/x.png", ""] {
        let r = store.create(&root, bad, &Draft::new(b"x"));
        assert!(matches!(r, Err(StoreError::BadPath { .. })), "{bad}: {r:?}");
    }
    for not_asset in ["x.txt", "x.png.mcmeta", "x.png.thumb.png", "noext"] {
        assert!(matches!(store.create(&root, not_asset, &Draft::new(b"x")), Err(StoreError::NotAnAsset(_))), "{not_asset}");
    }
    store.create(&root, "a.png", &Draft::new(b"x")).unwrap();
    assert!(matches!(store.move_to(&root, "a.png", &root, "../out"), Err(StoreError::BadPath { .. })));
    assert!(matches!(store.rename(&root, "a.png", " .. "), Err(StoreError::BadPath { .. })));
    #[cfg(unix)]
    {
        // A folder in the library that links elsewhere is outside it.
        fs::create_dir_all(dir.join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join("lib/link")).unwrap();
        assert!(matches!(store.create(&root, "link/x.png", &Draft::new(b"x")), Err(StoreError::BadPath { .. })));
        assert!(matches!(store.move_to(&root, "a.png", &root, "link"), Err(StoreError::BadPath { .. })));
        assert!(snapshot(&dir.join("elsewhere")).is_empty());
    }
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn read_only_and_in_memory_libraries_are_refused() {
    let dir = temp_dir("refuse-roots");
    let store = AssetStore::new();
    let user = user_root(&dir.join("user"));
    let starter = LibraryRoot::folder("starter", "Starter", RootKind::Starter, dir.join("starter"));
    let session = LibraryRoot::in_memory("session", "Session", RootKind::Session);
    fs::create_dir_all(dir.join("starter")).unwrap();
    fs::write(dir.join("starter/s.png"), b"s").unwrap();
    store.create(&user, "u.png", &Draft::new(b"u")).unwrap();
    let ro = |r: Result<_, StoreError>| matches!(r, Err(StoreError::ReadOnly { .. }));
    assert!(ro(store.create(&starter, "x.png", &Draft::new(b"x"))));
    assert!(ro(store.update(&starter, "s.png", &MetaEdit::default())));
    assert!(ro(store.rename(&starter, "s.png", "t")));
    assert!(ro(store.delete(&starter, "s.png")));
    assert!(ro(store.move_to(&starter, "s.png", &user, "")));
    assert!(ro(store.move_to(&user, "u.png", &starter, "")));
    assert!(matches!(store.create(&session, "x.png", &Draft::new(b"x")), Err(StoreError::NoFolder { .. })));
    assert!(matches!(store.delete(&user, "missing.png"), Err(StoreError::Missing(_))));
    assert_eq!(names(&snapshot(&dir)), ["starter/", "starter/s.png", "user/", "user/u.png"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_sidecar_that_does_not_read_is_never_rewritten() {
    let dir = temp_dir("refuse-sidecar");
    let store = AssetStore::new();
    let root = user_root(&dir);
    store.create(&root, "a.png", &Draft::new(b"a")).unwrap();
    fs::write(dir.join("a.png.mcmeta"), "{ not json").unwrap();
    let before = snapshot(&dir);
    assert!(matches!(store.update(&root, "a.png", &MetaEdit { name: Some("x".into()), ..MetaEdit::default() }), Err(StoreError::Sidecar { .. })));
    assert!(matches!(store.rename(&root, "a.png", "b"), Err(StoreError::Sidecar { .. })));
    assert_eq!(snapshot(&dir), before);
    // Deleting still works: the user asked for the asset to go.
    store.delete(&root, "a.png").unwrap();
    assert!(snapshot(&dir).is_empty());
    fs::remove_dir_all(&dir).unwrap();
}
