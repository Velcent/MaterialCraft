//! [`StoreRequest`]: a change to an existing asset as a value, naming libraries by id, so the editor can hand it to
//! the thread that owns the library ([`Library::run`](crate::assets::Library::run)).

use super::{AssetStore, MetaEdit, StoreChange, StoreError};
use crate::assets::{Catalogue, RootId};

/// A change to an asset in a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreRequest {
    Update { root: RootId, path: String, edit: MetaEdit },
    Rename { root: RootId, path: String, name: String },
    Move { root: RootId, path: String, to: RootId, folder: String },
    Delete { root: RootId, path: String },
}

impl StoreRequest {
    /// The asset the request is about.
    pub fn asset(&self) -> (&RootId, &str) {
        match self {
            StoreRequest::Update { root, path, .. }
            | StoreRequest::Rename { root, path, .. }
            | StoreRequest::Move { root, path, .. }
            | StoreRequest::Delete { root, path } => (root, path),
        }
    }

    /// What the request does, for the log ("Renamed", "Deleted" …).
    pub fn verb(&self) -> &'static str {
        match self {
            StoreRequest::Update { .. } => "Updated",
            StoreRequest::Rename { .. } => "Renamed",
            StoreRequest::Move { .. } => "Moved",
            StoreRequest::Delete { .. } => "Deleted",
        }
    }

    /// Runs the request with `store`, finding the libraries in `catalogue`.
    pub fn run(&self, store: &AssetStore, catalogue: &Catalogue) -> Result<StoreChange, StoreError> {
        let root = |id: &RootId| catalogue.root(id).ok_or_else(|| StoreError::UnknownLibrary(id.to_string()));
        match self {
            StoreRequest::Update { root: id, path, edit } => store.update(root(id)?, path, edit),
            StoreRequest::Rename { root: id, path, name } => store.rename(root(id)?, path, name),
            StoreRequest::Move { root: id, path, to, folder } => store.move_to(root(id)?, path, root(to)?, folder),
            StoreRequest::Delete { root: id, path } => store.delete(root(id)?, path),
        }
    }
}
