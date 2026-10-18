//! Create: a new asset's file, sidecar and thumbnail, written to hidden files first and renamed into place together.
//! Nothing existing is ever replaced.

use super::bundle::Bundle;
use super::fs::StoreFs;
use super::meta::SidecarText;
use super::place::Writable;
use super::transaction::atomically;
use super::{AssetRef, StoreChange, StoreError};
use crate::assets::naming::new_asset_name;
use crate::assets::{LibraryRoot, Sidecar, Usage};

/// The content of a new asset.
#[derive(Debug, Clone, Copy)]
pub struct Draft<'a> {
    /// The asset file's bytes.
    pub bytes: &'a [u8],
    /// Its sidecar, if it has one.
    pub sidecar: Option<&'a Sidecar>,
    /// Its conventional thumbnail (PNG), for assets that are not their own picture.
    pub thumbnail: Option<&'a [u8]>,
}

impl<'a> Draft<'a> {
    pub fn new(bytes: &'a [u8]) -> Draft<'a> {
        Draft { bytes, sidecar: None, thumbnail: None }
    }
}

/// Writes `draft` as the asset at library-relative `rel` of `root`.
pub(crate) fn create(fs: &dyn StoreFs, root: &LibraryRoot, rel: &str, draft: &Draft) -> Result<StoreChange, StoreError> {
    let w = Writable::of(root)?;
    let (file, _) = w.asset(rel)?;
    let bundle = Bundle::of(&file);
    if let Some(p) = bundle.any_existing(fs) {
        return Err(StoreError::Exists(p.to_path_buf()));
    }
    let ((), warnings) = atomically(fs, |tx| {
        if let Some(dir) = file.parent() {
            tx.ensure_dir(dir)?;
        }
        let file = tx.stage(&bundle.file, draft.bytes)?;
        let sidecar = draft.sidecar.map(|s| tx.stage(&bundle.sidecar, SidecarText::of(s).as_bytes())).transpose()?;
        let thumbnail = draft.thumbnail.map(|t| tx.stage(&bundle.thumbnail, t)).transpose()?;
        // The companions first: the asset shows up with its sidecar and thumbnail, never without.
        if let Some(s) = sidecar {
            tx.place(&s, &bundle.sidecar)?;
        }
        if let Some(t) = thumbnail {
            tx.place(&t, &bundle.thumbnail)?;
        }
        tx.place(&file, &bundle.file)
    })?;
    Ok(StoreChange { before: None, after: Some(AssetRef::new(root.id.clone(), rel)), warnings })
}

/// Writes `draft` into `usage`'s folder of `root` under a file name made from `name` (numbered when the name, its
/// sidecar or its thumbnail is taken).
pub(crate) fn create_named(fs: &dyn StoreFs, root: &LibraryRoot, usage: Usage, name: &str, extension: &str, draft: &Draft) -> Result<StoreChange, StoreError> {
    let w = Writable::of(root)?;
    let rel = new_asset_name(usage, name, extension, |rel| w.asset(rel).is_ok_and(|(file, _)| Bundle::of(&file).any_existing(fs).is_some()));
    create(fs, root, &rel, draft)
}
