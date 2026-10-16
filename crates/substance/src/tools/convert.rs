//! `mcsub convert`: import a `.spp` and save it as `.mcpaint`, for one file or a whole folder.

use crate::inspect::Corpus;
use crate::translate::{import_spp, ImportError};
use materialcraft::paint::format::{save_path, FormatError};
use materialcraft::paint::{Document, Fidelity};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Share of the progress bar for the import; saving takes the rest.
const IMPORT_SHARE: f32 = 0.85;

#[derive(Debug)]
pub enum ConvertError {
    Import(ImportError),
    Save(FormatError),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvertError::Import(e) => write!(f, "import failed: {e}"),
            ConvertError::Save(e) => write!(f, "save failed: {e}"),
        }
    }
}

impl std::error::Error for ConvertError {}

/// What one conversion produced.
#[derive(Debug, Clone)]
pub struct ConvertSummary {
    pub output: PathBuf,
    pub sets: usize,
    pub channels: usize,
    pub baked: usize,
    /// Report entries per fidelity: exact, approximated, baked, kept raw.
    pub fidelity: [usize; 4],
    pub bytes: u64,
    pub import_seconds: f64,
    pub save_seconds: f64,
}

impl ConvertSummary {
    fn new(doc: &Document, output: &Path, import_seconds: f64, save_seconds: f64) -> Self {
        let count = |f| doc.import_report.as_ref().map_or(0, |r| r.count(f));
        ConvertSummary {
            output: output.to_path_buf(),
            sets: doc.texture_sets.len(),
            channels: doc.texture_sets.iter().map(|s| s.channels.len()).sum(),
            baked: doc.texture_sets.iter().map(|s| s.baked.len()).sum(),
            fidelity: [
                Fidelity::Exact,
                Fidelity::Approximated,
                Fidelity::Baked,
                Fidelity::KeptRaw,
            ]
            .map(count),
            bytes: std::fs::metadata(output).map_or(0, |m| m.len()),
            import_seconds,
            save_seconds,
        }
    }
}

impl fmt::Display for ConvertSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [exact, approx, baked, raw] = self.fidelity;
        write!(
            f,
            "ok {}: {} texture sets, {} channels, {} baked maps; report {exact} exact, {approx} approximated, \
             {baked} baked, {raw} kept raw; {:.1} MB; import {:.1} s, save {:.1} s",
            self.output.display(),
            self.sets,
            self.channels,
            self.baked,
            self.bytes as f64 / 1048576.0,
            self.import_seconds,
            self.save_seconds
        )
    }
}

pub fn convert_one(
    input: &Path,
    output: &Path,
    progress: &mut dyn FnMut(f32, &str),
) -> Result<ConvertSummary, ConvertError> {
    let start = Instant::now();
    let doc = import_spp(input, &mut |f, m| progress(f * IMPORT_SHARE, m)).map_err(ConvertError::Import)?;
    let imported = start.elapsed().as_secs_f64();
    progress(IMPORT_SHARE, "saving");
    save_path(&doc, output).map_err(ConvertError::Save)?;
    let saved = start.elapsed().as_secs_f64() - imported;
    progress(1.0, "done");
    Ok(ConvertSummary::new(&doc, output, imported, saved))
}

/// Results of [`convert_all`], printable as a table.
pub struct ConvertTable {
    pub rows: Vec<(PathBuf, Result<ConvertSummary, String>)>,
    pub seconds: f64,
}

/// Converts every `.spp` below `root` into `out_dir` (`<n>.mcpaint`); `discard` deletes each output after
/// measuring it. `log` gets one line per file.
pub fn convert_all(root: &Path, out_dir: &Path, discard: bool, log: &mut dyn FnMut(&str)) -> ConvertTable {
    let start = Instant::now();
    let files = Corpus::discover(root).files().to_vec();
    let mut rows = Vec::with_capacity(files.len());
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        log(&format!("cannot create {}: {e}", out_dir.display()));
    }
    for (i, input) in files.iter().enumerate() {
        log(&format!("[{}/{}] {}", i + 1, files.len(), input.display()));
        let output = out_dir.join(format!("{i:03}.mcpaint"));
        let result = convert_one(input, &output, &mut |_, _| {}).map_err(|e| e.to_string());
        if discard {
            let _ = std::fs::remove_file(&output);
        }
        rows.push((input.strip_prefix(root).unwrap_or(input).to_path_buf(), result));
    }
    ConvertTable {
        rows,
        seconds: start.elapsed().as_secs_f64(),
    }
}

impl fmt::Display for ConvertTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ok: Vec<&ConvertSummary> = self.rows.iter().filter_map(|(_, r)| r.as_ref().ok()).collect();
        for (path, result) in &self.rows {
            match result {
                Ok(s) => writeln!(
                    f,
                    "ok    {:>6.1}s {:>6.1}s {:>8.1} MB  {:>2} sets  {}",
                    s.import_seconds,
                    s.save_seconds,
                    s.bytes as f64 / 1048576.0,
                    s.sets,
                    path.display()
                )?,
                Err(e) => writeln!(f, "FAIL  {}: {e}", path.display())?,
            }
        }
        let sum = |g: fn(&ConvertSummary) -> f64| ok.iter().map(|s| g(s)).sum::<f64>();
        writeln!(
            f,
            "{} converted, {} failed; import {:.0} s, save {:.0} s, output {:.0} MB; wall {:.0} s",
            ok.len(),
            self.rows.len() - ok.len(),
            sum(|s| s.import_seconds),
            sum(|s| s.save_seconds),
            sum(|s| s.bytes as f64) / 1048576.0,
            self.seconds
        )
    }
}
