//! One catalogue entry: an asset file (or an in-memory asset) with what the tray shows and searches.

use std::time::SystemTime;

use super::format::{file_name, folder_of};
use super::{Format, Provenance, Usage};

/// Where the tray gets an asset's picture from. Only a reference: thumbnails are loaded (or
/// rendered) lazily by the panel, never by the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Thumbnail {
    /// Nothing yet; the panel renders one or shows the usage icon.
    #[default]
    None,
    /// The asset is an image and is its own thumbnail.
    OwnImage,
    /// An image file, root-relative (`/`-separated).
    File(String),
    /// The asset file holds its own thumbnail (a fragment's `thumbnail.png` entry).
    Embedded,
}

/// An asset in a library.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    /// Root-relative path with `/` separators; unique within its root.
    pub path: String,
    /// Display name: the sidecar's name, else the file name without its extension.
    pub name: String,
    pub usage: Usage,
    pub format: Format,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub thumbnail: Thumbnail,
    /// Baked maps a smart material or mask needs (AS-05).
    pub needs: Vec<String>,
    pub provenance: Provenance,
    /// True if the asset has a sidecar that was read successfully.
    pub has_sidecar: bool,
    /// File modification time and size, `None` for in-memory assets.
    pub modified: Option<SystemTime>,
    pub size: u64,
    pub(crate) keys: SearchKeys,
}

impl Asset {
    /// A bare asset: name from the file name, no tags, unknown provenance.
    pub fn new(path: impl Into<String>, format: Format, usage: Usage) -> Asset {
        let path = path.into();
        let name = default_name(&path).to_owned();
        let thumbnail = if format.is_image() { Thumbnail::OwnImage } else { Thumbnail::None };
        let mut asset = Asset {
            path,
            name,
            usage,
            format,
            tags: Vec::new(),
            description: None,
            thumbnail,
            needs: Vec::new(),
            provenance: Provenance::Unknown,
            has_sidecar: false,
            modified: None,
            size: 0,
            keys: SearchKeys::default(),
        };
        asset.reindex();
        asset
    }

    /// The asset's folder, root-relative (empty at the root).
    pub fn folder(&self) -> &str {
        folder_of(&self.path)
    }

    /// The file name with extension.
    pub fn file_name(&self) -> &str {
        file_name(&self.path)
    }

    /// True if MaterialCraft can only offer to import it (`.sbsar`).
    pub fn is_importable_only(&self) -> bool {
        !self.format.is_native()
    }

    /// Recompute the lowercase search keys after changing fields. The catalogue calls this on insert.
    pub(crate) fn reindex(&mut self) {
        self.keys = SearchKeys::of(self);
    }
}

/// The asset's name without directory and extension.
pub(crate) fn default_name(path: &str) -> &str {
    let name = file_name(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Lowercased copies of the searchable fields, computed once so a query over thousands of assets
/// only compares strings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SearchKeys {
    pub name: String,
    pub folder: String,
    pub file: String,
    pub tags: Vec<String>,
}

impl SearchKeys {
    fn of(a: &Asset) -> SearchKeys {
        SearchKeys {
            name: a.name.to_lowercase(),
            folder: a.folder().to_lowercase(),
            file: a.file_name().to_lowercase(),
            tags: a.tags.iter().map(|t| t.to_lowercase()).collect(),
        }
    }
}
