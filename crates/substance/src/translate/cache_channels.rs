//! [`CacheChannels`]: the flattened channel images of one texture set, read from `/texture-cache/<set>/…`.
//! Each entry holds one channel of the whole layer stack; the last component is coverage (see
//! [`super::source_format`]). Rows are stored bottom first, as OpenGL textures (Verified against Painter exports).

use super::error::PartError;
use super::pixel_convert::{Conversion, RowOrder};
use super::source_format::SourceFormat;
use crate::decode::texture_cache::CacheTexture;
use crate::project::{CacheEntryRef, SppProject};
use materialcraft::paint::TiledImage;

/// Row order of texture-cache images.
const CACHE_ROWS: RowOrder = RowOrder::BottomFirst;

/// One decoded cache entry.
pub struct CacheImage {
    pub bit: u64,
    /// Straight values with coverage as alpha, rows top first.
    pub image: TiledImage,
    /// Bytes the stored LZ4 block was short by (padded with zeros).
    pub missing_bytes: usize,
    /// No texel has coverage below 1.
    pub fully_covered: bool,
}

/// The cache entries of a project, grouped by set on request.
pub struct CacheChannels<'a> {
    project: &'a SppProject,
    entries: Vec<CacheEntryRef>,
}

impl<'a> CacheChannels<'a> {
    pub fn new(project: &'a SppProject) -> Self {
        CacheChannels {
            project,
            entries: project.cache_entries(),
        }
    }

    /// Folder names (set uids) that have entries.
    pub fn sets(&self) -> std::collections::BTreeSet<String> {
        self.entries.iter().map(|e| e.set.clone()).collect()
    }

    /// Entries of one set, by ascending channel bit.
    pub fn entries_of(&self, set: &str) -> Vec<&CacheEntryRef> {
        let mut v: Vec<&CacheEntryRef> = self.entries.iter().filter(|e| e.set == set).collect();
        v.sort_by_key(|e| e.channel_bit);
        v
    }

    /// Reads and converts one entry.
    pub fn read(&self, entry: &CacheEntryRef) -> Result<CacheImage, PartError> {
        let data = self.project.read(&entry.path)?;
        let texture = CacheTexture::parse(&data)?;
        let source = SourceFormat::from_gl(texture.gl_internal_format)
            .ok_or_else(|| PartError::UnknownFormat(texture.gl_format_name().to_string()))?;
        let pixels = texture.decompress_padded()?;
        let conversion = Conversion {
            source,
            rows: CACHE_ROWS,
        };
        let converted = conversion.convert(texture.width, texture.height, &pixels.bytes)?;
        Ok(CacheImage {
            bit: entry.channel_bit,
            image: converted.image,
            missing_bytes: pixels.missing,
            fully_covered: converted.fully_covered,
        })
    }
}
