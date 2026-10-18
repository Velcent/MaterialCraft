//! [`StoreFs`]: the file operations the asset store is made of, behind a trait so tests can make any one of them
//! fail and check that the store rolls back. [`DiskFs`] is the real file system.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

/// The file operations of the asset store. Every write goes to a file that did not exist before.
pub trait StoreFs: Send + Sync {
    /// Creates one folder (its parent exists).
    fn create_dir(&self, dir: &Path) -> io::Result<()>;
    /// Removes an empty folder.
    fn remove_dir(&self, dir: &Path) -> io::Result<()>;
    /// Writes `bytes` into a new file and flushes it to disk; fails if `path` exists.
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;
    /// Copies `from` into a new file `to` and flushes it to disk; fails if `to` exists.
    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    /// Renames a file within one file system (replacing `to` where the platform does).
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    /// True if something (a file, folder or link) is at `path`.
    fn exists(&self, path: &Path) -> bool;
}

/// The real file system.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskFs;

impl StoreFs for DiskFs {
    fn create_dir(&self, dir: &Path) -> io::Result<()> {
        fs::create_dir(dir)
    }

    fn remove_dir(&self, dir: &Path) -> io::Result<()> {
        fs::remove_dir(dir)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = create_new(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    fn copy_new(&self, from: &Path, to: &Path) -> io::Result<()> {
        let mut source = File::open(from)?;
        let mut file = create_new(to)?;
        io::copy(&mut source, &mut file)?;
        file.sync_all()
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn exists(&self, path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok()
    }
}

fn create_new(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}
