//! Thumbnails kept on disk between runs (in the settings folder's `cache/thumbnails/`), so a material is rendered
//! once and not on every start. A file is named by a hash of the source path, its modification time and size and
//! the thumbnail version; a changed asset therefore gets a new file. Failing to read or write the cache only
//! means rendering again.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use materialcraft::png;
use materialcraft::tex::Tex;

use super::super::results::ThumbSource;
use super::render::THUMB_PX;

/// Bumped when thumbnails are made differently, so old files are not used.
const VERSION: u32 = 1;

/// The on-disk thumbnail cache; without a folder it does nothing.
#[derive(Clone, Debug, Default)]
pub struct DiskCache {
    dir: Option<PathBuf>,
}

impl DiskCache {
    pub fn new(dir: Option<PathBuf>) -> DiskCache {
        DiskCache { dir }
    }

    /// The cached thumbnail of `source` as of `stamp`.
    pub fn load(&self, source: &ThumbSource, stamp: (Option<SystemTime>, u64)) -> Option<Tex> {
        let path = self.file(source, stamp)?;
        let bytes = std::fs::read(path).ok()?;
        png::decode(&bytes).ok()
    }

    /// Keeps `tex` as the thumbnail of `source` as of `stamp`.
    pub fn store(&self, source: &ThumbSource, stamp: (Option<SystemTime>, u64), tex: &Tex) {
        let Some(path) = self.file(source, stamp) else { return };
        let Some(dir) = path.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, png::encode(tex.w, tex.h, &tex.px, 8)).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    fn file(&self, source: &ThumbSource, (modified, size): (Option<SystemTime>, u64)) -> Option<PathBuf> {
        let dir = self.dir.as_ref()?;
        let (kind, path): (u8, &Path) = match source {
            ThumbSource::None => return None,
            ThumbSource::Image(p) => (1, p),
            ThumbSource::Material(p) => (2, p),
            ThumbSource::Fragment(p) => (3, p),
        };
        let mut h = Fnv::new();
        h.write(&VERSION.to_le_bytes());
        h.write(&(THUMB_PX as u32).to_le_bytes());
        h.write(&[kind]);
        h.write(path.to_string_lossy().as_bytes());
        let nanos = modified.and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        h.write(&nanos.to_le_bytes());
        h.write(&size.to_le_bytes());
        Some(dir.join(format!("{:016x}.png", h.0)))
    }
}

/// FNV-1a, 64 bit: stable across runs.
struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_thumbnail_comes_back_until_the_file_changes() {
        let dir = std::env::temp_dir().join(format!("mc-tray-disk-{}", std::process::id()));
        let cache = DiskCache::new(Some(dir.clone()));
        let src = ThumbSource::Material(PathBuf::from("/lib/materials/rust.mca"));
        let t = Tex::solid(4, 4, [1.0, 0.0, 0.0, 1.0]);
        assert!(cache.load(&src, (None, 10)).is_none());
        cache.store(&src, (None, 10), &t);
        assert_eq!(cache.load(&src, (None, 10)).map(|l| (l.w, l.px[0])), Some((4, [1.0, 0.0, 0.0, 1.0])));
        assert!(cache.load(&src, (None, 11)).is_none(), "another size is another file");
        assert!(DiskCache::default().load(&src, (None, 10)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
