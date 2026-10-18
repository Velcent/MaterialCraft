//! Which files in a library are assets, sidecars or thumbnails (AS-23).
//!
//! Only MaterialCraft's own formats are assets: `.mca` graphs, PNG and OpenEXR images, Radiance `.hdr` environment
//! maps, `.mcfrag` fragments (smart materials and smart masks), `.mcbrush` brush presets, `.mctemplate` project templates. A `.sbsar`
//! file is recognised so the tray can show it as *importable*; it is never run (ADR 0008,
//! q-sbsar-in-library).

use std::fmt;

/// Suffix of an asset's sidecar, appended to the asset's full file name: `brick.mca.mcmeta`.
pub const SIDECAR_SUFFIX: &str = ".mcmeta";
/// Suffix of an asset's conventional thumbnail, appended to its full file name: `brick.mca.thumb.png`.
pub const THUMBNAIL_SUFFIX: &str = ".thumb.png";

/// The file format of an asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    /// A MaterialCraft node graph (`.mca`).
    Graph,
    /// A PNG image.
    Png,
    /// An OpenEXR image.
    Exr,
    /// A Radiance RGBE image (`.hdr`): an environment map.
    Hdr,
    /// A smart material or smart mask (`.mcfrag`, [`crate::paint::format::fragment`]); the file says which.
    Fragment,
    /// A brush preset (`.mcbrush`, [`crate::paint::brush::file`]).
    Brush,
    /// A project template (`.mctemplate`, [`crate::paint::format::template`]): defaults for new projects.
    Template,
    /// A Substance archive: shown as importable only, never shipped or run.
    Sbsar,
}

impl Format {
    /// Every format.
    pub const ALL: [Format; 8] = [Format::Graph, Format::Png, Format::Exr, Format::Hdr, Format::Fragment, Format::Brush, Format::Template, Format::Sbsar];

    /// The format of a file name, by its extension (case-insensitive).
    pub fn of(file_name: &str) -> Option<Format> {
        let (_, ext) = file_name.rsplit_once('.')?;
        let ext = ext.to_ascii_lowercase();
        Format::ALL.into_iter().find(|f| f.extension() == ext)
    }

    /// Lowercase extension without the dot; also the keyword of the `format:` query field.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Graph => "mca",
            Format::Png => "png",
            Format::Exr => "exr",
            Format::Hdr => "hdr",
            Format::Fragment => crate::paint::format::fragment::EXTENSION,
            Format::Brush => crate::paint::brush::file::EXTENSION,
            Format::Template => crate::paint::format::template::EXTENSION,
            Format::Sbsar => "sbsar",
        }
    }

    /// True if MaterialCraft can use the file directly; false for formats it can only offer to import.
    pub fn is_native(self) -> bool {
        self != Format::Sbsar
    }

    /// True for image formats, which are their own thumbnail.
    pub fn is_image(self) -> bool {
        matches!(self, Format::Png | Format::Exr | Format::Hdr)
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}

/// What a file found in a library is to the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole<'a> {
    /// An asset of this format.
    Asset(Format),
    /// The sidecar of the asset at this root-relative path.
    Sidecar { asset: &'a str },
    /// The conventional thumbnail of the asset at this root-relative path.
    Thumbnail { asset: &'a str },
    /// Anything else; ignored.
    Other,
}

impl<'a> FileRole<'a> {
    /// Classify a root-relative path. Sidecars and thumbnails are checked first, so `x.png.thumb.png`
    /// is a thumbnail, not an image asset.
    pub fn of(path: &'a str) -> FileRole<'a> {
        if let Some(asset) = strip_suffix_ignore_case(path, SIDECAR_SUFFIX) {
            return FileRole::Sidecar { asset };
        }
        if let Some(asset) = strip_suffix_ignore_case(path, THUMBNAIL_SUFFIX) {
            return FileRole::Thumbnail { asset };
        }
        match Format::of(file_name(path)) {
            Some(format) => FileRole::Asset(format),
            None => FileRole::Other,
        }
    }
}

/// The last segment of a `/`-separated path.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// The folder part of a `/`-separated path (empty at the root).
pub(crate) fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

fn strip_suffix_ignore_case<'a>(s: &'a str, suffix: &str) -> Option<&'a str> {
    let cut = s.len().checked_sub(suffix.len())?;
    let (head, tail) = (s.get(..cut)?, s.get(cut..)?);
    (tail.eq_ignore_ascii_case(suffix) && !head.is_empty() && !head.ends_with('/')).then_some(head)
}
