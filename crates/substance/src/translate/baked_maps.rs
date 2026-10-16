//! [`BakedMaps`]: the maps Painter baked from the mesh (AO, curvature, normal, …) as [`BakedMap`]s.
//!
//! The document lists them per texture set (`additionalMaps` and `normalMapBitmap`, by resource URL
//! `/<name>?version=<resource id>`, or `resourcealias:<Map>` when nothing is assigned). Sets the document
//! does not describe fall back to resource names of the form `<Map> from Mesh <set>` in `/resources.toc`.
//! Image resources are stored top row first.

use super::channel_table::baked_kind;
use super::error::PartError;
use super::pixel_convert::{Conversion, RowOrder};
use super::source_format::SourceFormat;
use crate::decode::document::BitmapRef;
use crate::decode::image::ImageResource;
use crate::decode::resource_toc::ResourceToc;
use crate::project::{paths, SppProject};
use materialcraft::paint::{BakedKind, BakedMap};

/// Resource-name prefixes of baked maps, for sets the document does not describe.
const NAME_PREFIXES: &[(&str, Option<u64>)] = &[
    ("Normal Map from Mesh ", None),
    ("Color Map from Mesh ", Some(0)),
    ("Ambient Occlusion Map from Mesh ", Some(1)),
    ("Curvature Map from Mesh ", Some(2)),
    ("World Space Normals ", Some(3)),
    ("Position ", Some(4)),
    ("Thickness Map from Mesh ", Some(5)),
];

/// A baked map to read: its kind, display name and resource id.
#[derive(Debug, Clone, PartialEq)]
pub struct BakedSource {
    pub kind: BakedKind,
    pub name: String,
    pub resource: String,
}

/// One converted map plus how many bytes its stored block was short by.
pub struct BakedImage {
    pub map: BakedMap,
    pub missing_bytes: usize,
    /// The dataset it was read from.
    pub dataset: String,
}

pub struct BakedMaps<'a> {
    project: &'a SppProject,
    toc: Option<ResourceToc>,
}

impl<'a> BakedMaps<'a> {
    pub fn new(project: &'a SppProject) -> Self {
        BakedMaps {
            project,
            toc: project.resource_toc().ok().flatten(),
        }
    }

    /// The baked maps of a set: from the document's references when it has any, else from resource names.
    /// References like `resourcealias:Normal` mean "no map assigned" and are skipped; Painter then ignores
    /// older bakes still in the file (Verified on `Zubzub.spp` / Planes, whose export shows no baked normal).
    pub fn sources(&self, set_name: &str, refs: &[BitmapRef]) -> Vec<BakedSource> {
        if !refs.is_empty() {
            return refs.iter().filter_map(from_ref).collect();
        }
        let Some(toc) = &self.toc else { return Vec::new() };
        toc.entries
            .iter()
            .filter_map(|e| {
                let usage = NAME_PREFIXES
                    .iter()
                    .find(|(prefix, _)| e.name.strip_prefix(prefix) == Some(set_name))?
                    .1;
                Some(BakedSource {
                    kind: baked_kind(usage),
                    name: e.name.clone(),
                    resource: e.version.clone()?,
                })
            })
            .collect()
    }

    pub fn read(&self, source: &BakedSource) -> Result<BakedImage, PartError> {
        let dataset = self.dataset(&source.resource)?;
        let data = self.project.read(&dataset)?;
        let image = ImageResource::parse(&data)?;
        let format = SourceFormat::from_image(&image.format)
            .ok_or_else(|| PartError::UnknownFormat(image.format.name().to_string()))?;
        let pixels = image.decompress_padded()?;
        let conversion = Conversion {
            source: format,
            rows: RowOrder::TopFirst,
        };
        Ok(BakedImage {
            map: BakedMap {
                kind: source.kind.clone(),
                image: conversion.convert(image.width, image.height, &pixels.bytes)?.image,
            },
            missing_bytes: pixels.missing,
            dataset,
        })
    }

    /// `/resources/<id>`, possibly with an extension Painter appended.
    fn dataset(&self, resource: &str) -> Result<String, PartError> {
        let exact = format!("{}/{resource}", paths::RESOURCES);
        let layout = self.project.layout();
        if layout.dataset(&exact).is_some() {
            return Ok(exact);
        }
        let prefix = format!("{exact}.");
        layout
            .datasets_under(paths::RESOURCES)
            .find(|d| d.path.starts_with(&prefix))
            .map(|d| d.path.to_string())
            .ok_or_else(|| PartError::Missing(format!("resource {resource}")))
    }
}

/// Splits `/<name>?version=<id>`.
fn from_ref(r: &BitmapRef) -> Option<BakedSource> {
    let (name, resource) = r.url.strip_prefix('/').unwrap_or(&r.url).rsplit_once("?version=")?;
    Some(BakedSource {
        kind: baked_kind(r.usage),
        name: name.to_string(),
        resource: resource.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_resource_urls() {
        let r = BitmapRef {
            url: "/Curvature Map from Mesh Cup?version=81ae".into(),
            usage: Some(2),
        };
        let s = from_ref(&r).unwrap();
        assert_eq!(
            (s.kind, s.name.as_str(), s.resource.as_str()),
            (BakedKind::Curvature, "Curvature Map from Mesh Cup", "81ae")
        );
        assert!(from_ref(&BitmapRef::default()).is_none());
        let alias = BitmapRef {
            url: "resourcealias:Normal".into(),
            usage: None,
        };
        assert!(from_ref(&alias).is_none());
    }
}
