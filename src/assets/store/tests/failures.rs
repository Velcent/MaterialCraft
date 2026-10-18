//! Interrupted operations: every operation is run again and again with its first, second, third … file operation
//! failing, until it succeeds. After each failure the libraries must be exactly as before (every file, byte for byte,
//! and no temporary files or new folders left).

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::assets::{AssetStore, DiskFs, MetaEdit, StoreChange, StoreError, StoreFs};

/// The real file system, with its `fail_at`-th changing operation (counting from 0) failing.
struct FailingFs {
    fail_at: usize,
    calls: AtomicUsize,
}

impl FailingFs {
    fn step(&self) -> io::Result<()> {
        match self.calls.fetch_add(1, Ordering::SeqCst) == self.fail_at {
            true => Err(io::Error::other("injected failure")),
            false => Ok(()),
        }
    }
}

impl StoreFs for FailingFs {
    fn create_dir(&self, dir: &Path) -> io::Result<()> {
        self.step()?;
        DiskFs.create_dir(dir)
    }
    fn remove_dir(&self, dir: &Path) -> io::Result<()> {
        DiskFs.remove_dir(dir)
    }
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        // A write that fails half-way leaves half a file behind.
        if self.step().is_err() {
            DiskFs.write_new(path, bytes.get(..bytes.len() / 2).unwrap_or_default())?;
            return Err(io::Error::other("injected failure"));
        }
        DiskFs.write_new(path, bytes)
    }
    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.step()?;
        DiskFs.copy_new(from, to)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        DiskFs.read(path)
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.step()?;
        DiskFs.rename(from, to)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.step()?;
        DiskFs.remove_file(path)
    }
    fn exists(&self, path: &Path) -> bool {
        DiskFs.exists(path)
    }
}

/// Runs `op` with failure after failure on fresh copies of the libraries `setup` makes, checking each failure left
/// them unchanged; returns how many steps failed before it went through.
fn interrupted(tag: &str, setup: impl Fn(&Path), op: impl Fn(&AssetStore, &Path) -> Result<StoreChange, StoreError>) -> usize {
    for fail_at in 0..64 {
        let dir = temp_dir(&format!("fail-{tag}"));
        setup(&dir);
        let before = snapshot(&dir);
        let store = AssetStore::with_fs(Box::new(FailingFs { fail_at, calls: AtomicUsize::new(0) }));
        match op(&store, &dir) {
            Err(e) => {
                assert!(matches!(e, StoreError::Io { .. }), "{tag}, step {fail_at}: {e}");
                assert_eq!(names(&snapshot(&dir)), names(&before), "{tag}: failing step {fail_at} changed the files");
                assert_eq!(snapshot(&dir), before, "{tag}: failing step {fail_at} changed bytes");
            }
            Ok(change) => {
                // Only removing the old files after success may fail; they are hidden and reported.
                let hidden = snapshot(&dir).keys().filter(|k| k.rsplit('/').next().is_some_and(|n| n.starts_with('.'))).count();
                assert!(hidden <= change.warnings.len(), "{tag}, step {fail_at}: {hidden} hidden files, {:?}", change.warnings);
                std::fs::remove_dir_all(&dir).unwrap();
                if hidden == 0 {
                    return fail_at;
                }
                continue;
            }
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    panic!("{tag} never succeeded");
}

fn two_libraries(dir: &Path) {
    let store = AssetStore::new();
    let mut side = sidecar("A");
    side.thumbnail = Some("a.png.thumb.png".into());
    store.create(&user_root(&dir.join("user")), "alphas/a.png", &full(&side)).unwrap();
    std::fs::create_dir_all(dir.join("extra")).unwrap();
}

fn user(dir: &Path) -> LibraryRoot {
    user_root(&dir.join("user"))
}

#[test]
fn every_failing_step_of_every_operation_leaves_the_libraries_as_they_were() {
    let side = sidecar("New");
    let steps = interrupted("create", two_libraries, |s, d| s.create(&user(d), "textures/new/deep/n.png", &full(&side)));
    assert!(steps >= 8, "folders, three staged files, three renames: {steps}");
    let steps = interrupted("update", two_libraries, |s, d| s.update(&user(d), "alphas/a.png", &MetaEdit { name: Some("B".into()), ..MetaEdit::default() }));
    assert!(steps >= 2, "{steps}");
    let steps = interrupted("rename", two_libraries, |s, d| s.rename(&user(d), "alphas/a.png", "Renamed"));
    assert!(steps >= 5, "{steps}");
    let steps = interrupted("move", two_libraries, |s, d| s.move_to(&user(d), "alphas/a.png", &user(d), "alphas/sub"));
    assert!(steps >= 4, "{steps}");
    let steps = interrupted("move-library", two_libraries, |s, d| s.move_to(&user(d), "alphas/a.png", &extra_root(&d.join("extra")), "x/y"));
    assert!(steps >= 10, "{steps}");
    let steps = interrupted("delete", two_libraries, |s, d| s.delete(&user(d), "alphas/a.png"));
    assert!(steps >= 3, "{steps}");
}
