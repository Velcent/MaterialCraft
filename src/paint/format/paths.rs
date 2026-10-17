//! Entry names inside the archive. Image names are derived from ids (made safe and unique);
//! readers never derive names, they use the paths recorded in the manifest.

use super::FormatError;
use super::images::Encoding;
use super::slot::ImageSlot;
use crate::paint::{Document, LayerImage, TiledImage};
use std::collections::{HashMap, HashSet};

pub const DOCUMENT: &str = "document.json";
pub const MESH: &str = "mesh/model.glb";
pub const IMPORT_REPORT: &str = "import-report.json";
pub const THUMBNAIL: &str = "thumbnail.png";
pub const FOREIGN_PREFIX: &str = "foreign/";

/// Longest path segment derived from an id.
const MAX_SEGMENT: usize = 64;

/// The entry name of every image of a document, in the order they are written.
pub struct ImagePlan<'d> {
    entries: Vec<(ImageSlot, String, &'d TiledImage)>,
    index: HashMap<ImageSlot, usize>,
    /// `(set index, layer id)` → the layer's directory, also used for its stroke records.
    layer_dirs: HashMap<(usize, String), String>,
}

impl<'d> ImagePlan<'d> {
    /// Names every image: `textures/<set>/layers/<layer>/<channel>.<ext>`,
    /// `textures/<set>/layers/<layer>/effect-<index>-<channel>.<ext>`,
    /// `textures/<set>/layers/<layer>/mask-<index>.<ext>` and `textures/<set>/baked/<kind>.<ext>`.
    pub fn new(doc: &'d Document) -> ImagePlan<'d> {
        let mut plan = ImagePlan { entries: Vec::new(), index: HashMap::new(), layer_dirs: HashMap::new() };
        let mut set_dirs = Names::default();
        for (s, set) in doc.texture_sets.iter().enumerate() {
            let dir = format!("textures/{}", set_dirs.claim(&segment(&set.id)));
            let mut layer_dirs = Names::default();
            for (_, layer) in set.walk_layers() {
                let ldir = format!("{dir}/layers/{}", layer_dirs.claim(&segment(&layer.id)));
                for (image, img) in layer.images() {
                    let stem = match image {
                        LayerImage::Paint(channel) => channel.id(),
                        LayerImage::Effect { effect, channel } => format!("effect-{effect}-{}", channel.id()),
                        LayerImage::Mask { effect } => format!("mask-{effect}"),
                    };
                    plan.push(ImageSlot::Layer { set: s, layer: layer.id.clone(), image }, format!("{ldir}/{stem}"), img);
                }
                plan.layer_dirs.insert((s, layer.id.clone()), ldir);
            }
            let mut baked_names = Names::default();
            for (i, b) in set.baked.iter().enumerate() {
                let stem = baked_names.claim(&segment(b.kind.id()));
                plan.push(ImageSlot::Baked { set: s, index: i }, format!("{dir}/baked/{stem}"), &b.image);
            }
        }
        plan
    }

    fn push(&mut self, slot: ImageSlot, stem: String, img: &'d TiledImage) {
        let path = format!("{stem}.{}", Encoding::for_format(img.format()).extension());
        // Stems cannot collide: channel ids never start with "mask-" or "effect-".
        self.index.insert(slot.clone(), self.entries.len());
        self.entries.push((slot, path, img));
    }

    /// The entry name of `slot`.
    pub fn path(&self, slot: &ImageSlot) -> Option<&str> {
        self.index.get(slot).map(|&i| self.entries[i].1.as_str())
    }

    /// The directory of layer `layer` of set `set` (`textures/<set>/layers/<layer>`).
    pub fn layer_dir(&self, set: usize, layer: &str) -> Option<&str> {
        self.layer_dirs.get(&(set, layer.to_string())).map(String::as_str)
    }

    /// `(entry name, image)` in writing order.
    pub fn images(&self) -> impl Iterator<Item = (&str, &'d TiledImage)> + '_ {
        self.entries.iter().map(|(_, p, i)| (p.as_str(), *i))
    }
}

/// Unique names within one directory: a repeat gets `-2`, `-3`, ….
#[derive(Default)]
struct Names(HashSet<String>);

impl Names {
    fn claim(&mut self, name: &str) -> String {
        let mut candidate = name.to_string();
        let mut n = 1;
        while !self.0.insert(candidate.clone()) {
            n += 1;
            candidate = format!("{name}-{n}");
        }
        candidate
    }
}

/// A path segment for `id`: ASCII letters, digits, `-`, `_` and `.` kept, everything else `_`;
/// at most 64 bytes; never empty, `.` or `..`.
pub fn segment(id: &str) -> String {
    let s: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).take(MAX_SEGMENT).collect();
    if s.is_empty() || s.chars().all(|c| c == '.') { format!("_{s}") } else { s }
}

/// Checks a foreign file path: under `foreign/`, `/`-separated, no empty, `.` or `..` segments,
/// no backslashes.
pub fn check_foreign_path(path: &str) -> Result<(), FormatError> {
    let rest = path.strip_prefix(FOREIGN_PREFIX);
    let ok = rest.is_some_and(|r| !r.contains('\\') && r.split('/').all(|s| !s.is_empty() && s != "." && s != ".."));
    if ok { Ok(()) } else { Err(FormatError::Unsupported(format!("foreign file path {path:?} must look like \"foreign/<source>/<file>\""))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_safe() {
        assert_eq!(segment("Base Color"), "Base_Color");
        assert_eq!(segment("../x"), ".._x");
        assert_eq!(segment(".."), "_..");
        assert_eq!(segment(""), "_");
        assert_eq!(segment("Röst/1"), "R_st_1");
        assert_eq!(segment(&"a".repeat(100)).len(), 64);
        let mut n = Names::default();
        assert_eq!(n.claim("a"), "a");
        assert_eq!(n.claim("a"), "a-2");
        assert_eq!(n.claim("a"), "a-3");
    }

    #[test]
    fn foreign_paths() {
        assert!(check_foreign_path("foreign/substance/document.bin").is_ok());
        for bad in ["substance/x", "foreign/", "foreign//x", "foreign/../x", "foreign/a\\b", "/foreign/x"] {
            assert!(check_foreign_path(bad).is_err(), "{bad}");
        }
    }
}
