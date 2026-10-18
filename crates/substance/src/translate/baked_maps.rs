//! [`BakedMaps`]: the maps Painter baked from the mesh (AO, curvature, normal, …) as [`BakedMap`]s.
//!
//! The document lists them per texture set (`additionalMaps` and `normalMapBitmap`, by resource URL
//! `/<name>?version=<resource id>`, or `resourcealias:<Map>` when nothing is assigned). Sets the document
//! does not describe fall back to resource names of the form `<Map> from Mesh <set>` in `/resources.toc`.
//! Image resources are stored top row first.

use super::channel_table::baked_kind;
use super::error::PartError;
use super::resources::{read_image, split_resource_url};
use crate::decode::document::BitmapRef;
use crate::decode::resource_toc::ResourceToc;
use crate::project::SppProject;
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

/// True for a resource name Painter gives the maps it bakes (`Ambient Occlusion Map from Mesh Cup`), whatever the set.
pub(crate) fn is_baked_name(name: &str) -> bool {
    NAME_PREFIXES.iter().any(|(prefix, _)| name.starts_with(prefix))
}

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
        let read = read_image(self.project, &source.resource)?;
        Ok(BakedImage {
            map: BakedMap {
                kind: source.kind.clone(),
                image: read.image,
            },
            missing_bytes: read.missing_bytes,
            dataset: read.dataset,
        })
    }
}

/// A baked-map reference to an embedded resource (`/<name>?version=<id>`).
fn from_ref(r: &BitmapRef) -> Option<BakedSource> {
    let (name, resource) = split_resource_url(&r.url)?;
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
