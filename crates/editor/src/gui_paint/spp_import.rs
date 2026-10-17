//! [`SppImport`]: imports a Substance Painter project in-process through `materialcraft-substance`
//! (ADR 0003) and keeps the result in the conversion cache, so reopening an unchanged project is instant.
//!
//! The importer has no cancel hook yet: a cancelled import runs to the end on its background thread and
//! its result is then dropped (nothing is cached).

use super::cache;
use super::job::CancelToken;
use materialcraft::paint::Document;
use materialcraft::paint::format::save_path;
use materialcraft_substance::translate::import_spp;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Share of the progress bar for the import; the rest is saving to the cache.
const IMPORT_SHARE: f32 = 0.9;

/// An imported (or cached) document and a one-line summary for the import report's tooltip.
pub struct Imported {
    pub document: Document,
    pub summary: String,
}

pub struct SppImport {
    /// The running editor, which contains the importer: part of the cache key, so a new build imports again.
    build: PathBuf,
}

impl SppImport {
    pub fn new() -> SppImport {
        SppImport { build: std::env::current_exe().unwrap_or_default() }
    }

    /// Opens the cached conversion of `spp`, or imports it and caches the result.
    pub fn open(&self, spp: &Path, cancel: &CancelToken, progress: &mut dyn FnMut(f32, &str)) -> Result<Imported, String> {
        let cached = cache::converted_path(spp, &self.build);
        if cached.is_file() {
            progress(IMPORT_SHARE, "Reading the cached conversion");
            let document = materialcraft::paint::format::load_path(&cached).map_err(|e| format!("Cannot read {}: {e}", cached.display()))?;
            return Ok(Imported { document, summary: "Opened the cached conversion.".into() });
        }
        let start = Instant::now();
        let document = import_spp(spp, &mut |f, m| progress(f * IMPORT_SHARE, m)).map_err(|e| format!("Import failed: {e}"))?;
        if cancel.is_cancelled() {
            return Err("Import cancelled.".into());
        }
        let imported = start.elapsed().as_secs_f64();
        progress(IMPORT_SHARE, "Saving to the conversion cache");
        let stored = std::fs::create_dir_all(cache::dir()).map_err(|e| e.to_string()).and_then(|()| save_path(&document, &cached).map_err(|e| e.to_string()));
        let summary = summary(&document, imported, stored.err());
        Ok(Imported { document, summary })
    }
}

fn summary(doc: &Document, seconds: f64, cache_error: Option<String>) -> String {
    let channels: usize = doc.texture_sets.iter().map(|s| s.channels.len()).sum();
    let baked: usize = doc.texture_sets.iter().map(|s| s.baked.len()).sum();
    let mut line = format!("imported in {seconds:.1} s: {} texture sets, {channels} channels, {baked} baked maps", doc.texture_sets.len());
    if let Some(e) = cache_error {
        line.push_str(&format!("; not cached: {e}"));
    }
    line
}
