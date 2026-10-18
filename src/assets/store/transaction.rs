//! [`Transaction`]: the steps of one store operation, each recorded with how to undo it. New content is written to
//! hidden temporary files next to its target first and then renamed into place; files that go away are renamed to
//! hidden names first and only removed once every step succeeded. If a step fails, [`atomically`] undoes the steps
//! before it in reverse order, so the library is left as it was. Hidden files (a leading `.`) are never listed, so
//! the catalogue never sees a half-done operation.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::StoreError;
use super::fs::StoreFs;

/// Numbers the hidden files of this process.
static NEXT: AtomicU64 = AtomicU64::new(0);

enum Undo {
    RemoveFile(PathBuf),
    RemoveDir(PathBuf),
    RenameBack { now: PathBuf, was: PathBuf },
}

/// The steps done so far.
pub(crate) struct Transaction<'a> {
    fs: &'a dyn StoreFs,
    undo: Vec<Undo>,
    /// Files renamed out of the way, removed on commit.
    trash: Vec<PathBuf>,
}

/// Runs `steps` as one transaction: commits when they succeed, undoes them when one fails. Returns the result and
/// warnings about hidden files a commit could not remove (the operation itself succeeded).
pub(crate) fn atomically<T>(fs: &dyn StoreFs, steps: impl FnOnce(&mut Transaction) -> Result<T, StoreError>) -> Result<(T, Vec<String>), StoreError> {
    let mut tx = Transaction { fs, undo: Vec::new(), trash: Vec::new() };
    match steps(&mut tx) {
        Ok(value) => Ok((value, tx.commit())),
        Err(cause) => match tx.rollback() {
            leftovers if leftovers.is_empty() => Err(cause),
            leftovers => Err(StoreError::Incomplete { cause: Box::new(cause), leftovers }),
        },
    }
}

impl Transaction<'_> {
    /// Creates `dir` and its missing parents; each created folder is removed again on rollback.
    pub fn ensure_dir(&mut self, dir: &Path) -> Result<(), StoreError> {
        let mut missing = Vec::new();
        let mut d = Some(dir);
        while let Some(p) = d.filter(|p| !p.as_os_str().is_empty() && !self.fs.exists(p)) {
            missing.push(p.to_path_buf());
            d = p.parent();
        }
        for p in missing.into_iter().rev() {
            self.fs.create_dir(&p).map_err(|e| StoreError::io(&p, e))?;
            self.undo.push(Undo::RemoveDir(p));
        }
        Ok(())
    }

    /// Writes `bytes` to a hidden file next to `target`; returns it for [`Transaction::place`].
    pub fn stage(&mut self, target: &Path, bytes: &[u8]) -> Result<PathBuf, StoreError> {
        let temp = hidden(target, "new");
        self.undo.push(Undo::RemoveFile(temp.clone()));
        self.fs.write_new(&temp, bytes).map_err(|e| StoreError::io(&temp, e))?;
        Ok(temp)
    }

    /// Copies `source` to a hidden file next to `target` (which may be on another file system).
    pub fn stage_copy(&mut self, source: &Path, target: &Path) -> Result<PathBuf, StoreError> {
        let temp = hidden(target, "new");
        self.undo.push(Undo::RemoveFile(temp.clone()));
        self.fs.copy_new(source, &temp).map_err(|e| StoreError::io(&temp, e))?;
        Ok(temp)
    }

    /// Renames `from` to `to`, which must not exist.
    pub fn place(&mut self, from: &Path, to: &Path) -> Result<(), StoreError> {
        if self.fs.exists(to) {
            return Err(StoreError::Exists(to.to_path_buf()));
        }
        self.fs.rename(from, to).map_err(|e| StoreError::io(to, e))?;
        self.undo.push(Undo::RenameBack { now: to.to_path_buf(), was: from.to_path_buf() });
        Ok(())
    }

    /// Renames `from` over the existing `to` in one step. It can't be undone, so it is only ever an operation's last
    /// step: if it fails, `to` is unchanged.
    pub fn replace_last(&mut self, from: &Path, to: &Path) -> Result<(), StoreError> {
        self.fs.rename(from, to).map_err(|e| StoreError::io(to, e))
    }

    /// Renames `path` to a hidden name; it is removed on commit and comes back on rollback.
    pub fn trash(&mut self, path: &Path) -> Result<(), StoreError> {
        let gone = hidden(path, "old");
        self.fs.rename(path, &gone).map_err(|e| StoreError::io(path, e))?;
        self.undo.push(Undo::RenameBack { now: gone.clone(), was: path.to_path_buf() });
        self.trash.push(gone);
        Ok(())
    }

    /// Removes the trashed files; returns those it could not remove (hidden, so the library doesn't list them).
    fn commit(self) -> Vec<String> {
        let fs = self.fs;
        self.trash.iter().filter_map(|p| fs.remove_file(p).err().map(|e| format!("{} could not be removed: {e}", p.display()))).collect()
    }

    /// Undoes every step, newest first; returns what could not be undone.
    fn rollback(self) -> Vec<String> {
        let mut leftovers = Vec::new();
        for step in self.undo.into_iter().rev() {
            let (path, result) = match &step {
                Undo::RemoveFile(p) if self.fs.exists(p) => (p, self.fs.remove_file(p)),
                Undo::RemoveFile(_) => continue,
                Undo::RemoveDir(p) => (p, self.fs.remove_dir(p)),
                Undo::RenameBack { now, was } => (now, self.fs.rename(now, was)),
            };
            if let Err(e) = result {
                leftovers.push(format!("{} ({e})", path.display()));
            }
        }
        leftovers
    }
}

/// A hidden name next to `path`: `.<file name>.<process>-<n>.<tag>`.
fn hidden(path: &Path, tag: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}-{n}.mcstore-{tag}", std::process::id()))
}
