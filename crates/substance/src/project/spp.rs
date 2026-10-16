//! [`SppProject`]: one opened `.spp` file. A thin façade: it finds datasets and hands their bytes to the
//! decoders in [`crate::decode`]; listing logic lives in [`super::layout`], [`super::resources`] and [`super::cache`].

use super::cache::CacheEntryRef;
use super::error::ProjectError;
use super::layout::Layout;
use super::paths;
use super::resources::ResourceRef;
use crate::container::hdf5::Hdf5File;
use crate::decode::assbin::AssbinScene;
use crate::decode::document::{ClassCensus, DocumentMaterials, DocumentSettings, MaterialRecord};
use crate::decode::resource_toc::ResourceToc;
use crate::decode::settings::ProjectSettings;
use std::path::{Path, PathBuf};

pub struct SppProject {
    path: PathBuf,
    file: Hdf5File,
    layout: Layout,
}

impl SppProject {
    pub fn open(path: &Path) -> Result<Self, ProjectError> {
        let file = Hdf5File::open(path)?;
        let layout = Layout::new(file.entries()?);
        if layout.dataset(paths::DOCUMENT).is_none() {
            return Err(ProjectError::NotAPainterProject {
                missing: paths::DOCUMENT,
            });
        }
        Ok(SppProject {
            path: path.to_path_buf(),
            file,
            layout,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Raw bytes of any dataset.
    pub fn read(&self, dataset: &str) -> Result<Vec<u8>, ProjectError> {
        Ok(self.file.read(dataset)?)
    }

    /// The first `max_len` bytes of a dataset.
    pub fn read_prefix(&self, dataset: &str, max_len: usize) -> Result<Vec<u8>, ProjectError> {
        Ok(self.file.read_prefix(dataset, max_len)?)
    }

    /// `None` for projects that predate `projectsettings.ini`.
    pub fn settings(&self) -> Result<Option<ProjectSettings>, ProjectError> {
        self.decode_optional(paths::PROJECT_SETTINGS, ProjectSettings::parse)
    }

    pub fn resource_toc(&self) -> Result<Option<ResourceToc>, ProjectError> {
        self.decode_optional(paths::RESOURCE_TOC, ResourceToc::parse)
    }

    pub fn resources(&self) -> Vec<ResourceRef> {
        ResourceRef::all_in(&self.layout)
    }

    pub fn cache_entries(&self) -> Vec<CacheEntryRef> {
        CacheEntryRef::all_in(&self.layout)
    }

    pub fn mesh(&self) -> Result<AssbinScene, ProjectError> {
        let data = self.read(paths::SCENE)?;
        AssbinScene::parse(&data).map_err(ProjectError::decode(paths::SCENE))
    }

    pub fn document_census(&self) -> Result<ClassCensus, ProjectError> {
        let data = self.read(paths::DOCUMENT)?;
        ClassCensus::scan(&data).map_err(ProjectError::decode(paths::DOCUMENT))
    }

    /// The texture sets recorded in `document.bin`, in document order.
    pub fn document_materials(&self) -> Result<Vec<MaterialRecord>, ProjectError> {
        let data = self.read(paths::DOCUMENT)?;
        DocumentMaterials::scan(&data).map_err(ProjectError::decode(paths::DOCUMENT))
    }

    /// Format version and project-wide settings of `document.bin`.
    pub fn document_settings(&self) -> Result<DocumentSettings, ProjectError> {
        let data = self.read(paths::DOCUMENT)?;
        DocumentSettings::scan(&data).map_err(ProjectError::decode(paths::DOCUMENT))
    }

    fn decode_optional<T>(
        &self,
        dataset: &'static str,
        decode: impl FnOnce(&[u8]) -> Result<T, crate::decode::DecodeError>,
    ) -> Result<Option<T>, ProjectError> {
        if self.layout.dataset(dataset).is_none() {
            return Ok(None);
        }
        let data = self.read(dataset)?;
        decode(&data).map(Some).map_err(ProjectError::decode(dataset))
    }
}
