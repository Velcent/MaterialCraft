//! Runs `inspect` over every `.spp` below a folder and condenses each result into one table row.

use super::report::{InspectOptions, ProjectReport};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

/// One file's result in the corpus table.
#[derive(Debug, Clone)]
pub struct CorpusRow {
    pub path: PathBuf,
    pub outcome: Result<RowFacts, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFacts {
    pub file_bytes: u64,
    pub version: Option<String>,
    pub meshes: Option<usize>,
    pub texture_sets: Option<usize>,
    /// Images whose header decoded, out of all images.
    pub images: Option<(usize, usize)>,
    pub cache_entries: Option<usize>,
    pub document_objects: Option<usize>,
    pub failed_sections: usize,
}

impl RowFacts {
    fn from_report(r: &ProjectReport) -> RowFacts {
        let settings = r.settings.as_ref().ok().and_then(|s| s.as_ref());
        RowFacts {
            file_bytes: r.file_bytes,
            version: settings.and_then(|s| s.last_saved_with).map(|v| v.to_string()),
            meshes: r.mesh.as_ref().ok().map(|m| m.meshes.len()),
            texture_sets: r.mesh.as_ref().ok().map(|m| m.materials.len()),
            images: r.resources.as_ref().ok().map(|s| {
                let total = s
                    .kinds
                    .rows()
                    .iter()
                    .find(|(k, ..)| *k == "image")
                    .map_or(0, |(_, n, _)| *n);
                (total - s.unreadable_headers.len(), total)
            }),
            cache_entries: r.cache.as_ref().ok().map(|c| c.channels.total()),
            document_objects: r.document.as_ref().ok().map(|d| d.total()),
            failed_sections: r.failed_sections(),
        }
    }
}

pub struct Corpus {
    files: Vec<PathBuf>,
}

impl Corpus {
    /// Finds `.spp` files below `root`, sorted. Unreadable folders are skipped.
    pub fn discover(root: &Path) -> Corpus {
        let mut files = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(read) = std::fs::read_dir(&dir) else { continue };
            for entry in read.flatten() {
                let path = entry.path();
                match entry.file_type() {
                    Ok(t) if t.is_dir() => stack.push(path),
                    Ok(t) if t.is_file() && is_spp(&path) => files.push(path),
                    _ => {}
                }
            }
        }
        files.sort();
        Corpus { files }
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Inspects each file; `progress` is called before each one with (index, total, path).
    /// A panic inside a decoder is caught and recorded as that file's failure.
    pub fn run(&self, options: InspectOptions, mut progress: impl FnMut(usize, usize, &Path)) -> Vec<CorpusRow> {
        let total = self.files.len();
        self.files
            .iter()
            .enumerate()
            .map(|(i, path)| {
                progress(i, total, path);
                let result = panic::catch_unwind(AssertUnwindSafe(|| ProjectReport::build(path, options)));
                let outcome = match result {
                    Ok(Ok(report)) => Ok(RowFacts::from_report(&report)),
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(_) => Err("decoder panicked".to_string()),
                };
                CorpusRow {
                    path: path.clone(),
                    outcome,
                }
            })
            .collect()
    }
}

fn is_spp(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("spp"))
}
