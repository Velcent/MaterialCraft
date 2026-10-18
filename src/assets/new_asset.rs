//! [`NewAsset`]: an asset made outside the library (copied out of an imported project, roadmap step 46) written into
//! a library folder: the file in its usage's folder under a name that replaces nothing ([`new_asset_path`]), its
//! sidecar with name, usage, tags, description and provenance, and its thumbnail when it has one. The tray picks it
//! up on its next poll.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Provenance, SIDECAR_SUFFIX, Sidecar, THUMBNAIL_SUFFIX, Usage, new_asset_path};

/// An asset to write: its bytes and what its sidecar says.
#[derive(Debug, Clone, PartialEq)]
pub struct NewAsset {
    pub usage: Usage,
    /// The display name; the file name is made from it.
    pub name: String,
    /// The file's extension without the dot (`png`, `exr`, `mcbrush`).
    pub extension: String,
    pub bytes: Vec<u8>,
    /// A PNG thumbnail (`<file>.thumb.png`), for assets that are not their own picture.
    pub thumbnail: Option<Vec<u8>>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub provenance: Provenance,
}

/// Where a [`NewAsset`] was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedAsset {
    pub path: PathBuf,
    /// The path relative to the library, `/`-separated (`alphas/scratch.png`): how presets and documents refer to it.
    pub reference: String,
}

impl NewAsset {
    /// The sidecar written next to the file.
    pub fn sidecar(&self) -> Sidecar {
        Sidecar {
            name: Some(self.name.clone()),
            usage: Some(self.usage),
            tags: self.tags.clone(),
            description: self.description.clone(),
            provenance: self.provenance.clone(),
            ..Sidecar::default()
        }
    }

    /// Writes the asset into `library` (its usage's folder is created when missing). An existing file is never
    /// replaced: the name gets a number.
    pub fn save(&self, library: &Path) -> io::Result<SavedAsset> {
        let path = new_asset_path(library, self.usage, &self.name, &self.extension);
        let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("no file name for {}", self.name)));
        };
        fs::create_dir_all(dir)?;
        fs::write(&path, &self.bytes)?;
        fs::write(dir.join(format!("{file}{SIDECAR_SUFFIX}")), format!("{}\n", self.sidecar().to_json().to_string_pretty()))?;
        if let Some(png) = &self.thumbnail {
            fs::write(dir.join(format!("{file}{THUMBNAIL_SUFFIX}")), png)?;
        }
        Ok(SavedAsset { reference: format!("{}/{file}", self.usage.folder()), path })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::assets::{Library, LibraryRoot, RootId, RootKind, Thumbnail};

    #[test]
    fn a_saved_asset_lands_in_its_usage_folder_with_its_sidecar_and_shows_in_the_catalogue() {
        let dir = std::env::temp_dir().join(format!("materialcraft-new-asset-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let asset = NewAsset {
            usage: Usage::Alpha,
            name: "Scratch: 1".into(),
            extension: "png".into(),
            bytes: crate::png::encode(1, 1, &[[1.0; 4]], 8),
            thumbnail: None,
            tags: vec!["imported".into()],
            description: Some("From Cup.spp".into()),
            provenance: Provenance::Imported { project: "Cup.spp".into(), origin: "embedded in the project".into(), shareable: true },
        };
        let saved = asset.save(&dir).unwrap();
        assert_eq!(saved.reference, "alphas/Scratch_ 1.png");
        assert_eq!(saved.path, dir.join("alphas").join("Scratch_ 1.png"));
        let again = asset.save(&dir).unwrap();
        assert_eq!(again.reference, "alphas/Scratch_ 1 2.png", "nothing is replaced");

        let lib = Library::with_roots([LibraryRoot::folder("user", "My assets", RootKind::User, &dir)], &crate::diag::NullSink);
        let found = lib.catalogue().get(&RootId::new("user"), "alphas/Scratch_ 1.png").unwrap();
        assert_eq!((found.name.as_str(), found.usage, &found.thumbnail), ("Scratch: 1", Usage::Alpha, &Thumbnail::OwnImage));
        assert!(found.provenance.is_imported() && found.has_sidecar);
        fs::remove_dir_all(&dir).unwrap();
    }
}
