//! Move and delete. A move within one library renames the asset's files into the other folder; a move into another
//! library copies them there (it may be on another disk) and removes the originals once every copy is in place.
//! A delete renames the files out of sight first and removes them only when all of them went.

use super::bundle::Bundle;
use super::edit::existing;
use super::fs::StoreFs;
use super::meta::SidecarText;
use super::place::{Writable, join};
use super::transaction::{Transaction, atomically};
use super::{AssetRef, StoreChange, StoreError};
use crate::assets::build::resolve_relative;
use crate::assets::format::{file_name, folder_of};
use crate::assets::{LibraryRoot, THUMBNAIL_SUFFIX};
use crate::json::Value;
use std::path::Path;

/// Moves the asset at `rel` of `from` into `folder` (library-relative, empty for the top) of `to`, keeping its file
/// name. Refused when the target is taken.
pub(crate) fn move_to(fs: &dyn StoreFs, from: &LibraryRoot, rel: &str, to: &LibraryRoot, folder: &str) -> Result<StoreChange, StoreError> {
    let (src, dst) = (Writable::of(from)?, Writable::of(to)?);
    let file = existing(fs, &src, rel)?;
    let dir = dst.folder(folder)?;
    let new_rel = join(folder, file_name(rel));
    let (new_file, _) = dst.asset(&new_rel)?;
    let (old, new) = (Bundle::of(&file), Bundle::of(&new_file));
    if let Some(p) = new.any_existing(fs) {
        return Err(StoreError::Exists(p.to_path_buf()));
    }
    let present = old.present(fs);
    let same_root = from.id == to.id;
    let mut warnings = Vec::new();
    // A sidecar naming its own picture by a relative path gets the path from the new folder.
    let sidecar = match present.sidecar {
        true => {
            let mut v = SidecarText::read(fs, &old.sidecar, true)?;
            match repoint_thumbnail(&mut v, rel, &new_rel, same_root) {
                Repointed::Unchanged => None,
                Repointed::Changed => Some(SidecarText::of_json(&v)),
                Repointed::Dropped(t) => {
                    warnings.push(format!("{new_rel}: its picture {t} stays in the library {}", from.name));
                    Some(SidecarText::of_json(&v))
                }
            }
        }
        false => None,
    };
    let carry = |tx: &mut Transaction, old: &Path, new: &Path| match same_root {
        true => tx.place(old, new),
        false => {
            let staged = tx.stage_copy(old, new)?;
            tx.place(&staged, new)?;
            tx.trash(old)
        }
    };
    let ((), mut done) = atomically(fs, |tx| {
        tx.ensure_dir(&dir)?;
        if present.sidecar {
            match &sidecar {
                Some(text) => {
                    let staged = tx.stage(&new.sidecar, text.as_bytes())?;
                    tx.place(&staged, &new.sidecar)?;
                    tx.trash(&old.sidecar)?;
                }
                None => carry(tx, &old.sidecar, &new.sidecar)?,
            }
        }
        if present.thumbnail {
            carry(tx, &old.thumbnail, &new.thumbnail)?;
        }
        carry(tx, &old.file, &new.file)
    })?;
    warnings.append(&mut done);
    Ok(StoreChange { before: Some(AssetRef::new(from.id.clone(), rel)), after: Some(AssetRef::new(to.id.clone(), new_rel)), warnings })
}

/// Removes the asset at `rel` of `root` with its sidecar and thumbnail.
pub(crate) fn delete(fs: &dyn StoreFs, root: &LibraryRoot, rel: &str) -> Result<StoreChange, StoreError> {
    let w = Writable::of(root)?;
    let bundle = Bundle::of(&existing(fs, &w, rel)?);
    let present = bundle.present(fs);
    let ((), warnings) = atomically(fs, |tx| {
        tx.trash(&bundle.file)?;
        if present.sidecar {
            tx.trash(&bundle.sidecar)?;
        }
        if present.thumbnail {
            tx.trash(&bundle.thumbnail)?;
        }
        Ok(())
    })?;
    Ok(StoreChange { before: Some(AssetRef::new(root.id.clone(), rel)), after: None, warnings })
}

enum Repointed {
    Unchanged,
    Changed,
    /// The picture is in the old library; the sidecar no longer names it.
    Dropped(String),
}

/// Makes the sidecar's relative `thumbnail` path (of the asset moving from `rel` to `new_rel`) point at the same
/// picture from the new folder: the asset's own conventional thumbnail moves along; another picture is pointed at
/// where it is (within one library) or no longer named (from another library).
fn repoint_thumbnail(v: &mut Value, rel: &str, new_rel: &str, same_root: bool) -> Repointed {
    let Some(t) = v.get("thumbnail").and_then(Value::as_str).map(str::to_owned) else { return Repointed::Unchanged };
    let (old_folder, new_folder) = (folder_of(rel), folder_of(new_rel));
    let target = resolve_relative(old_folder, &t);
    let own = format!("{rel}{THUMBNAIL_SUFFIX}");
    let new_target = match target {
        Some(p) if p == own => format!("{new_rel}{THUMBNAIL_SUFFIX}"),
        Some(p) if same_root => p,
        _ => {
            v.remove("thumbnail");
            return Repointed::Dropped(t);
        }
    };
    let new_t = relative_from(new_folder, &new_target);
    if new_t == t {
        return Repointed::Unchanged;
    }
    v.insert("thumbnail", new_t);
    Repointed::Changed
}

/// The relative path from `folder` to the root-relative `target`.
fn relative_from(folder: &str, target: &str) -> String {
    let from: Vec<&str> = folder.split('/').filter(|s| !s.is_empty()).collect();
    let to: Vec<&str> = target.split('/').filter(|s| !s.is_empty()).collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let ups = std::iter::repeat_n("..", from.len().saturating_sub(common));
    ups.chain(to.iter().skip(common).copied()).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::relative_from;

    #[test]
    fn relative_paths_between_folders() {
        assert_eq!(relative_from("alphas", "alphas/x.png"), "x.png");
        assert_eq!(relative_from("alphas/old", "pictures/x.png"), "../../pictures/x.png");
        assert_eq!(relative_from("", "a/x.png"), "a/x.png");
    }
}
