//! Turns one listed asset file into an [`Asset`]: usage from sidecar, folder or format; sidecar
//! metadata; thumbnail reference. Problems go to the sink and never stop the scan.

use std::fs;
use std::path::Path;

use super::format::{SIDECAR_SUFFIX, THUMBNAIL_SUFFIX, folder_of};
use super::{Asset, Format, Listing, Sidecar, Thumbnail, Usage};
use crate::diag::{Area, Level, Sink, SinkExt, Target};
use crate::paint::FragmentKind;
use crate::paint::format::fragment;

/// Sidecars larger than this are not read (they are tiny; anything bigger is a mistake).
pub const MAX_SIDECAR_BYTES: u64 = 256 * 1024;

/// Builds assets of one folder-backed root.
pub(crate) struct AssetBuilder<'a> {
    pub root_dir: &'a Path,
    pub listing: &'a Listing,
    pub sink: &'a dyn Sink,
}

impl AssetBuilder<'_> {
    /// The asset at root-relative `path` of `format`, which the listing contains.
    pub fn build(&self, path: &str, format: Format) -> Asset {
        let usage = usage_from_folder(path).unwrap_or_else(|| default_usage(format));
        let mut asset = Asset::new(path, format, usage);
        if let Some(stamp) = self.listing.get(path) {
            asset.modified = stamp.modified;
            asset.size = stamp.len;
        }
        if format == Format::Fragment {
            self.apply_fragment(&mut asset);
        }
        let thumb = format!("{path}{THUMBNAIL_SUFFIX}");
        if self.listing.contains(&thumb) {
            asset.thumbnail = Thumbnail::File(thumb);
        }
        let sidecar_path = format!("{path}{SIDECAR_SUFFIX}");
        if self.listing.contains(&sidecar_path)
            && let Some(sidecar) = self.read_sidecar(&sidecar_path)
        {
            self.apply(&mut asset, sidecar);
        }
        asset.reindex();
        asset
    }

    /// What a fragment's own header says (AS-05): its usage (smart material or smart mask, whatever folder it
    /// sits in), name, needed baked maps and embedded thumbnail. A sidecar still overrides each of them.
    fn apply_fragment(&self, asset: &mut Asset) {
        match fragment::read_info_path(&self.root_dir.join(&asset.path)) {
            Ok(info) => {
                asset.usage = match info.kind {
                    FragmentKind::SmartMaterial => Usage::SmartMaterial,
                    FragmentKind::SmartMask => Usage::SmartMask,
                };
                if !info.name.trim().is_empty() {
                    asset.name = info.name;
                }
                asset.needs = info.needs.iter().map(|k| k.id().to_string()).collect();
                if info.thumbnail.is_some() {
                    asset.thumbnail = Thumbnail::Embedded;
                }
            }
            Err(e) => self.sink.about(Level::Warning, Area::Assets, Target::Asset(asset.path.clone()), format!("can't read the fragment: {e}")),
        }
    }

    fn read_sidecar(&self, rel: &str) -> Option<Sidecar> {
        let report = |text: String| self.sink.about(Level::Error, Area::Assets, Target::Asset(rel.to_owned()), text);
        if self.listing.get(rel).is_some_and(|s| s.len > MAX_SIDECAR_BYTES) {
            report(format!("sidecar is larger than {} KiB; ignored", MAX_SIDECAR_BYTES / 1024));
            return None;
        }
        let text = match fs::read_to_string(self.root_dir.join(rel)) {
            Ok(t) => t,
            Err(e) => {
                report(format!("can't read sidecar: {e}"));
                return None;
            }
        };
        match Sidecar::parse(&text) {
            Ok(s) => Some(s),
            Err(e) => {
                report(format!("malformed sidecar, ignored: {}", e.0));
                None
            }
        }
    }

    fn apply(&self, asset: &mut Asset, s: Sidecar) {
        asset.has_sidecar = true;
        if let Some(name) = s.name.filter(|n| !n.trim().is_empty()) {
            asset.name = name;
        }
        if let Some(usage) = s.usage {
            asset.usage = usage;
        }
        asset.tags = s.tags;
        asset.description = s.description;
        if !s.needs.is_empty() {
            asset.needs = s.needs;
        }
        asset.provenance = s.provenance;
        if let Some(t) = s.thumbnail {
            match resolve_relative(folder_of(&asset.path), &t) {
                Some(p) => asset.thumbnail = Thumbnail::File(p),
                None => self.sink.about(
                    Level::Warning,
                    Area::Assets,
                    Target::Asset(asset.path.clone()),
                    format!("thumbnail {t:?} points outside the library; ignored"),
                ),
            }
        }
    }
}

/// The usage named by the nearest enclosing folder, if any (AS-03).
pub(crate) fn usage_from_folder(path: &str) -> Option<Usage> {
    folder_of(path).rsplit('/').find_map(Usage::from_name)
}

/// The usage of a file whose folder and sidecar say nothing.
pub(crate) fn default_usage(format: Format) -> Usage {
    match format {
        Format::Graph | Format::Sbsar => Usage::Material,
        Format::Png | Format::Exr => Usage::Texture,
        Format::Hdr => Usage::Environment,
        Format::Fragment => Usage::SmartMaterial,
    }
}

/// Join `rel` (from a sidecar) onto `folder`, resolving `.` and `..`; `None` if it is absolute or
/// leaves the root.
pub(crate) fn resolve_relative(folder: &str, rel: &str) -> Option<String> {
    let rel = rel.replace('\\', "/");
    if rel.starts_with('/') || rel.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = folder.split('/').filter(|p| !p.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}
