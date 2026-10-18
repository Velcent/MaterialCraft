//! Update and rename: a new sidecar for an asset's name, tags and description, and a new file name for the asset with
//! its sidecar and thumbnail renamed along.

use std::path::PathBuf;

use super::bundle::Bundle;
use super::fs::StoreFs;
use super::meta::{MetaEdit, SidecarText};
use super::place::{Writable, join};
use super::transaction::atomically;
use super::{AssetRef, StoreChange, StoreError};
use crate::assets::LibraryRoot;
use crate::assets::format::{file_name, folder_of};
use crate::assets::naming::safe_stem;

/// The existing asset at `rel` of `root`: its file.
pub(crate) fn existing(fs: &dyn StoreFs, w: &Writable, rel: &str) -> Result<PathBuf, StoreError> {
    let (file, _) = w.asset(rel)?;
    match fs.exists(&file) {
        true => Ok(file),
        false => Err(StoreError::Missing(file)),
    }
}

/// Rewrites the sidecar of the asset at `rel` with `edit` applied (writing one if it has none).
pub(crate) fn update(fs: &dyn StoreFs, root: &LibraryRoot, rel: &str, edit: &MetaEdit) -> Result<StoreChange, StoreError> {
    let w = Writable::of(root)?;
    let bundle = Bundle::of(&existing(fs, &w, rel)?);
    let had = bundle.present(fs).sidecar;
    let mut v = SidecarText::read(fs, &bundle.sidecar, had)?;
    edit.apply(&mut v);
    let text = SidecarText::of_json(&v);
    let ((), warnings) = atomically(fs, |tx| {
        let staged = tx.stage(&bundle.sidecar, text.as_bytes())?;
        match had {
            true => tx.replace_last(&staged, &bundle.sidecar),
            false => tx.place(&staged, &bundle.sidecar),
        }
    })?;
    let here = AssetRef::new(root.id.clone(), rel);
    Ok(StoreChange { before: Some(here.clone()), after: Some(here), warnings })
}

/// Gives the asset at `rel` the display name `name` and a file name made from it (same folder and extension); its
/// sidecar and thumbnail are renamed along. Refused when the new file name is taken.
pub(crate) fn rename(fs: &dyn StoreFs, root: &LibraryRoot, rel: &str, name: &str) -> Result<StoreChange, StoreError> {
    let w = Writable::of(root)?;
    let file = existing(fs, &w, rel)?;
    let stem = safe_stem(name).ok_or_else(|| StoreError::bad_path(name, "is not a usable name"))?;
    let new_rel = join(folder_of(rel), &with_stem(file_name(rel), &stem));
    let rename_meta = MetaEdit { name: Some(name.to_owned()), ..MetaEdit::default() };
    if new_rel == rel {
        return update(fs, root, rel, &rename_meta);
    }
    let (new_file, _) = w.asset(&new_rel)?;
    let (old, new) = (Bundle::of(&file), Bundle::of(&new_file));
    if let Some(p) = new.any_existing(fs) {
        return Err(StoreError::Exists(p.to_path_buf()));
    }
    let present = old.present(fs);
    let mut v = SidecarText::read(fs, &old.sidecar, present.sidecar)?;
    rename_meta.apply(&mut v);
    let text = SidecarText::of_json(&v);
    let ((), warnings) = atomically(fs, |tx| {
        let staged = tx.stage(&new.sidecar, text.as_bytes())?;
        tx.place(&staged, &new.sidecar)?;
        if present.thumbnail {
            tx.place(&old.thumbnail, &new.thumbnail)?;
        }
        tx.place(&old.file, &new.file)?;
        if present.sidecar {
            tx.trash(&old.sidecar)?;
        }
        Ok(())
    })?;
    Ok(StoreChange { before: Some(AssetRef::new(root.id.clone(), rel)), after: Some(AssetRef::new(root.id.clone(), new_rel)), warnings })
}

/// `file` with its stem replaced by `stem`, keeping the extension as written.
fn with_stem(file: &str, stem: &str) -> String {
    match file.rsplit_once('.') {
        Some((_, ext)) => format!("{stem}.{ext}"),
        None => stem.to_owned(),
    }
}
