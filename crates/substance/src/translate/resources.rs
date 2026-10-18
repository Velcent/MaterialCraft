//! Embedded resources by URL: Painter refers to its own image resources as `/<name>?version=<id>`, where
//! `<id>` names the dataset `/resources/<id>` (sometimes with an extension Painter appended). Shelf
//! resources (`resource://<shelf>/<name>?version=<id>`) are usually not stored in the project, but Painter
//! often embeds the images it uses under the same `/resources/<id>` ([`shelf_version`]).

use super::error::PartError;
use super::pixel_convert::{Conversion, RowOrder};
use super::source_format::SourceFormat;
use crate::decode::image::ImageResource;
use crate::project::{SppProject, paths};
use materialcraft::paint::TiledImage;

/// One decoded image resource: straight values, rows top first (as image resources are stored).
pub struct ResourceImage {
    pub image: TiledImage,
    /// True for float formats, whose values are linear; integer formats hold encoded values.
    pub float: bool,
    /// Bytes the stored LZ4 block was short by (padded with zeros).
    pub missing_bytes: usize,
    /// The dataset it was read from.
    pub dataset: String,
}

/// Reads and converts the image resource `/resources/<id>`.
pub fn read_image(project: &SppProject, id: &str) -> Result<ResourceImage, PartError> {
    let dataset = resource_dataset(project, id)?;
    let data = project.read(&dataset)?;
    let image = ImageResource::parse(&data)?;
    let format = SourceFormat::from_image(&image.format)
        .ok_or_else(|| PartError::UnknownFormat(image.format.name().to_string()))?;
    let pixels = image.decompress_padded()?;
    let conversion = Conversion {
        source: format,
        rows: RowOrder::TopFirst,
    };
    Ok(ResourceImage {
        image: conversion.convert(image.width, image.height, &pixels.bytes)?.image,
        float: format.target().sample.is_float(),
        missing_bytes: pixels.missing,
        dataset,
    })
}

/// Splits `/<name>?version=<id>` into name and resource id; `None` for other URLs.
pub fn split_resource_url(url: &str) -> Option<(&str, &str)> {
    if url.starts_with("resource://") || url.starts_with("resourcealias:") {
        return None;
    }
    let (name, id) = url.strip_prefix('/').unwrap_or(url).rsplit_once("?version=")?;
    (!id.is_empty()).then_some((name, id))
}

/// The version id of a shelf URL (`resource://<shelf>/<name>?version=<id>`): the name of the dataset under
/// `/resources` when Painter embedded the resource in the project. `None` for other URLs.
pub fn shelf_version(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("resource://")?;
    let (_, id) = rest.rsplit_once("?version=")?;
    (!id.is_empty() && !id.contains('/')).then_some(id)
}

/// `/resources/<id>`, possibly with an extension Painter appended.
pub fn resource_dataset(project: &SppProject, id: &str) -> Result<String, PartError> {
    let exact = format!("{}/{id}", paths::RESOURCES);
    let layout = project.layout();
    if layout.dataset(&exact).is_some() {
        return Ok(exact);
    }
    let prefix = format!("{exact}.");
    layout
        .datasets_under(paths::RESOURCES)
        .find(|d| d.path.starts_with(&prefix))
        .map(|d| d.path.to_string())
        .ok_or_else(|| PartError::Missing(format!("resource {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_embedded_urls_only() {
        assert_eq!(
            split_resource_url("/Curvature Map from Mesh Cup?version=81ae"),
            Some(("Curvature Map from Mesh Cup", "81ae"))
        );
        assert_eq!(split_resource_url("resource://shelf/a?version=1.sbsar"), None);
        assert_eq!(split_resource_url("resourcealias:Normal"), None);
        assert_eq!(split_resource_url("/x?version="), None);
        assert_eq!(split_resource_url(""), None);
    }

    #[test]
    fn shelf_urls_name_their_version() {
        assert_eq!(
            shelf_version("resource://shelf/Cutlery1?version=c51e.image"),
            Some("c51e.image")
        );
        assert_eq!(
            shelf_version("resource://allegorithmic.95ac/stitches_a?version=b8b5.image"),
            Some("b8b5.image")
        );
        assert_eq!(shelf_version("resource://shelf/a"), None);
        assert_eq!(shelf_version("resource://shelf/a?version="), None);
        assert_eq!(shelf_version("/x?version=1"), None);
        assert_eq!(shelf_version("resourcealias:Normal"), None);
    }
}
