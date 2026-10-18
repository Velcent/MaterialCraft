//! Alpha-image tips (AS-06, PT-11): a preset's [`Tip::Alpha`] names a greyscale image by its library path; an
//! [`AlphaSource`] turns that name into a [`TipAlpha`] for the engine. [`FolderAlphas`] looks it up under the asset
//! library folders, the way material references are found, and keeps what it decoded until the file changes.
//! Round and procedural tips need no source: the engine makes them itself.

use crate::engine::TipAlpha;
use materialcraft::paint::brush::Tip;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Longest side of an alpha image used as a tip; larger images are refused (a tip is scaled to the dab anyway).
pub const MAX_ALPHA_SIDE: usize = 4096;

/// Where alpha-image tips come from.
pub trait AlphaSource {
    /// The alpha at library path `reference`, if it can be found and read.
    fn alpha(&self, reference: &str) -> Option<TipAlpha>;
}

/// No alpha images at all (alpha tips paint round).
pub struct NoAlphas;

impl AlphaSource for NoAlphas {
    fn alpha(&self, _: &str) -> Option<TipAlpha> {
        None
    }
}

/// The sampled tip a preset's `tip` needs from outside the engine: the alpha image of an alpha tip, else `None`
/// (round and procedural tips are made by the engine).
pub fn alpha_for(tip: &Tip, alphas: &dyn AlphaSource) -> Option<TipAlpha> {
    match tip {
        Tip::Alpha(reference) => alphas.alpha(reference),
        Tip::Round | Tip::Procedural(_) => None,
    }
}

/// An alpha image's coverage: white paints, black doesn't; colour images by their luminance, transparency lowers
/// coverage. PNG only (alphas are PNGs).
pub fn decode_alpha(bytes: &[u8]) -> Result<TipAlpha, String> {
    let tex = materialcraft::png::decode(bytes)?;
    if tex.w == 0 || tex.h == 0 || tex.w > MAX_ALPHA_SIDE || tex.h > MAX_ALPHA_SIDE {
        return Err(format!("{} × {} pixels is not a usable tip size (at most {MAX_ALPHA_SIDE})", tex.w, tex.h));
    }
    let values = tex.px.iter().map(|p| ((0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]) * p[3]).clamp(0.0, 1.0)).collect();
    TipAlpha::new(tex.w as u32, tex.h as u32, values).ok_or_else(|| "the image's pixel count does not match its size".to_string())
}

/// Alphas found under library folders: `reference` is tried under each folder in order. Decoded tips are kept
/// until their file's modification time or size changes.
#[derive(Default)]
pub struct FolderAlphas {
    folders: Vec<PathBuf>,
    cache: Mutex<HashMap<PathBuf, Cached>>,
}

struct Cached {
    stamp: (Option<SystemTime>, u64),
    tip: Option<TipAlpha>,
}

impl FolderAlphas {
    pub fn new(folders: Vec<PathBuf>) -> FolderAlphas {
        FolderAlphas { folders, cache: Mutex::default() }
    }

    /// Looks in `folders` from now on (the cache stays: it is keyed by file).
    pub fn set_folders(&mut self, folders: Vec<PathBuf>) {
        self.folders = folders;
    }

    /// The file `reference` names, if it lies inside one of the folders and exists.
    pub fn locate(&self, reference: &str) -> Option<PathBuf> {
        let rel = Path::new(reference);
        // library paths only: nothing absolute and no way out of the folder
        if rel.is_absolute() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            return None;
        }
        self.folders.iter().map(|f| f.join(rel)).find(|p| p.is_file())
    }

    fn load(&self, path: &Path) -> Option<TipAlpha> {
        let meta = std::fs::metadata(path).ok()?;
        let stamp = (meta.modified().ok(), meta.len());
        if let Some(c) = self.cache.lock().ok().and_then(|c| c.get(path).filter(|c| c.stamp == stamp).map(|c| c.tip.clone())) {
            return c;
        }
        let tip = std::fs::read(path).ok().and_then(|b| decode_alpha(&b).ok());
        if let Ok(mut c) = self.cache.lock() {
            c.insert(path.to_path_buf(), Cached { stamp, tip: tip.clone() });
        }
        tip
    }
}

impl AlphaSource for FolderAlphas {
    fn alpha(&self, reference: &str) -> Option<TipAlpha> {
        self.load(&self.locate(reference)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::deflate::Level;
    use materialcraft::png::{ColorType, encode_raw};

    #[test]
    fn alphas_are_found_under_the_folders_and_read_as_coverage() {
        let dir = std::env::temp_dir().join(format!("mc-brush-alphas-{}", std::process::id()));
        let (a, b) = (dir.join("a"), dir.join("b"));
        std::fs::create_dir_all(b.join("alphas")).unwrap();
        std::fs::create_dir_all(&a).unwrap();
        // 2 × 1 grey + alpha: white opaque, white half transparent
        let png = encode_raw(2, 1, ColorType::GrayAlpha, 8, &[255, 255, 255, 128], Level::Default);
        std::fs::write(b.join("alphas/dot.png"), &png).unwrap();
        let alphas = FolderAlphas::new(vec![a.clone(), b.clone()]);
        let tip = alphas.alpha("alphas/dot.png").unwrap();
        assert_eq!((tip.width, tip.height), (2, 1));
        assert!((tip.values[0] - 1.0).abs() < 1e-6 && (tip.values[1] - 128.0 / 255.0).abs() < 1e-3, "{:?}", tip.values);
        assert_eq!(alpha_for(&Tip::Alpha("alphas/dot.png".into()), &alphas), Some(tip));
        assert_eq!(alpha_for(&Tip::Procedural("chalk".into()), &alphas), None, "the engine makes procedural tips");
        assert!(alphas.alpha("alphas/none.png").is_none());
        assert!(alphas.alpha("../b/alphas/dot.png").is_none(), "no way out of a library folder");
        assert!(alphas.alpha(b.join("alphas/dot.png").to_str().unwrap()).is_none(), "no absolute paths");
        std::fs::write(b.join("alphas/dot.png"), b"not a png at all").unwrap();
        assert!(alphas.alpha("alphas/dot.png").is_none(), "a changed file is read again");
        assert!(NoAlphas.alpha("alphas/dot.png").is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
