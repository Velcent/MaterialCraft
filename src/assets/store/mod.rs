//! The asset store (roadmap step 47): the one place that writes to an asset library. An asset is one object made of
//! up to three files: the asset file, its sidecar (`<file>.mcmeta`) and its thumbnail (`<file>.thumb.png`). The
//! store creates, updates, renames, moves and deletes assets, always all three files together:
//!
//! - every operation is a [`transaction`]: new content goes to hidden temporary files and is renamed into place;
//!   files that go away are renamed out of sight and only removed when every step worked; a failing step undoes
//!   the steps before it, so the library is left as it was;
//! - nothing is ever replaced (a taken name is refused) and every path stays inside its library ([`place`]);
//! - the starter library is read-only, and in-memory roots have no folder: both are refused;
//! - each operation returns a [`StoreChange`] that [`Library::apply`](crate::assets::Library::apply) puts into the
//!   catalogue at once.
//!
//! ```text
//! AssetStore::create/update/rename/move_to/delete --> Transaction (stage, place, trash; undo on failure)
//!        |                                                   |
//!        +--> StoreChange --> Library::apply --> Watcher::restat + Catalogue::update
//! ```

mod bundle;
mod change;
mod create;
mod edit;
mod error;
mod fs;
mod meta;
mod place;
mod relocate;
mod request;
mod transaction;

#[cfg(test)]
mod tests;

pub use change::{AssetRef, StoreChange};
pub use create::Draft;
pub use error::StoreError;
pub use fs::{DiskFs, StoreFs};
pub use meta::MetaEdit;
pub use request::StoreRequest;

use super::{LibraryRoot, Usage};

/// Writes to asset libraries.
pub struct AssetStore {
    fs: Box<dyn StoreFs>,
}

impl Default for AssetStore {
    fn default() -> AssetStore {
        AssetStore::new()
    }
}

impl AssetStore {
    /// A store on the real file system.
    pub fn new() -> AssetStore {
        AssetStore { fs: Box::new(DiskFs) }
    }

    /// A store on `fs` (tests make its operations fail).
    pub fn with_fs(fs: Box<dyn StoreFs>) -> AssetStore {
        AssetStore { fs }
    }

    /// Writes a new asset at library-relative `path` of `root`. Refused if any of its three files exists.
    pub fn create(&self, root: &LibraryRoot, path: &str, draft: &Draft) -> Result<StoreChange, StoreError> {
        create::create(&*self.fs, root, path, draft)
    }

    /// Writes a new asset into `usage`'s folder of `root`, its file name made from `name` (`Rust: worn` gives
    /// `Rust_ worn.<extension>`) and numbered while taken.
    pub fn create_named(&self, root: &LibraryRoot, usage: Usage, name: &str, extension: &str, draft: &Draft) -> Result<StoreChange, StoreError> {
        create::create_named(&*self.fs, root, usage, name, extension, draft)
    }

    /// Changes the name, tags or description in the asset's sidecar (writing one if it has none). Fields the edit
    /// leaves alone, and fields this version doesn't know, are kept.
    pub fn update(&self, root: &LibraryRoot, path: &str, edit: &MetaEdit) -> Result<StoreChange, StoreError> {
        edit::update(&*self.fs, root, path, edit)
    }

    /// Names the asset `name`: the sidecar's name, and a file name made from it in the same folder.
    pub fn rename(&self, root: &LibraryRoot, path: &str, name: &str) -> Result<StoreChange, StoreError> {
        edit::rename(&*self.fs, root, path, name)
    }

    /// Moves the asset into `folder` (library-relative, empty for the top) of `to`, which may be another library.
    pub fn move_to(&self, from: &LibraryRoot, path: &str, to: &LibraryRoot, folder: &str) -> Result<StoreChange, StoreError> {
        relocate::move_to(&*self.fs, from, path, to, folder)
    }

    /// Removes the asset with its sidecar and thumbnail. Documents keep their embedded copies of it.
    pub fn delete(&self, root: &LibraryRoot, path: &str) -> Result<StoreChange, StoreError> {
        relocate::delete(&*self.fs, root, path)
    }
}
