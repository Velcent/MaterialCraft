//! [`ForeignFiles`]: the parts of the `.spp` we keep byte for byte under `foreign/substance/`, so a better
//! importer can convert the document again without the original file: `document.bin`, the mesh, settings,
//! `.sbsar`/shader resources, resource images that are not baked maps (brush alphas, stencils, bitmaps of
//! fills), metadata and the cache index. Left out: the texture-cache images and baked maps, which were
//! translated into the document.

use super::error::PartError;
use crate::project::{paths, SppProject};
use materialcraft::paint::ForeignFile;
use std::collections::BTreeSet;

/// Folder inside the document.
pub const FOREIGN_ROOT: &str = "foreign/substance";

/// What was kept.
#[derive(Debug, Default)]
pub struct KeptFiles {
    pub files: Vec<ForeignFile>,
    pub bytes: u64,
    /// Datasets that could not be read, with the reason.
    pub failed: Vec<(String, String)>,
}

pub struct ForeignFiles<'a> {
    project: &'a SppProject,
}

impl<'a> ForeignFiles<'a> {
    pub fn new(project: &'a SppProject) -> Self {
        ForeignFiles { project }
    }

    /// Reads every dataset except cache images and the `translated` datasets (absolute paths).
    pub fn collect(&self, translated: &BTreeSet<String>) -> KeptFiles {
        let mut kept = KeptFiles::default();
        for d in self.project.layout().datasets() {
            if translated.contains(d.path) || is_cache_image(d.path) {
                continue;
            }
            match self.read(d.path) {
                Ok(file) => {
                    kept.bytes += file.bytes.len() as u64;
                    kept.files.push(file);
                }
                Err(e) => kept.failed.push((d.path.to_string(), e.to_string())),
            }
        }
        kept
    }

    fn read(&self, dataset: &str) -> Result<ForeignFile, PartError> {
        Ok(ForeignFile {
            path: foreign_path(dataset),
            bytes: self.project.read(dataset)?,
        })
    }
}

/// `foreign/substance/<dataset path>`.
pub fn foreign_path(dataset: &str) -> String {
    format!("{FOREIGN_ROOT}/{}", dataset.trim_start_matches('/'))
}

/// Numeric leaves below `/texture-cache` (the `header` and `list` index datasets are kept).
fn is_cache_image(path: &str) -> bool {
    path.strip_prefix(paths::TEXTURE_CACHE).is_some_and(|rest| {
        rest.starts_with('/') && rest.rsplit('/').next().is_some_and(|leaf| leaf.parse::<u64>().is_ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_images_are_numeric_leaves() {
        assert!(is_cache_image("/texture-cache/12/3864292196/128"));
        assert!(!is_cache_image("/texture-cache/12/3864292196/list"));
        assert!(!is_cache_image("/texture-cache/header"));
        assert!(!is_cache_image("/paint/document.bin"));
        assert_eq!(
            foreign_path("/paint/document.bin"),
            "foreign/substance/paint/document.bin"
        );
    }
}
