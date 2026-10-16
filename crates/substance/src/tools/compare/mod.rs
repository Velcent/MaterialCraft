//! `mcsub compare`: our final maps against the textures Painter exported, per texture set and map, measured
//! inside the UV islands only (Painter pads exports beyond the islands; we do not).

mod convention;
mod export_image;
mod exports;
mod heatmap;
mod metrics;
mod painter_normals;
mod table;
mod uv_mask;

pub use exports::{ExportFinder, ExportMap};
pub use metrics::{Diff, Fit, TOLERANCE};
pub use painter_normals::PainterNormals;
pub use table::{CompareRow, CompareTable};
pub use uv_mask::UvMask;

use crate::tools::maps::file_safe;
use export_image::{flipped, load_export, resampled};
use materialcraft::paint::{Document, MaterialMaps, OutputOptions, TextureSet};
use materialcraft::tex::Tex;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Texels next to island borders left out of the comparison.
const BORDER: usize = 1;

pub struct CompareOptions {
    /// Modification time of the project; exports older than it may show an earlier state.
    pub project_time: Option<SystemTime>,
    /// Where to write diff heatmaps, if wanted.
    pub diff_dir: Option<PathBuf>,
    /// How our final maps are built. The height filter and baked normal format are replaced by the
    /// project's own (see [`PainterNormals`]) when its `document.bin` was kept.
    pub output: OutputOptions,
}

impl CompareOptions {
    pub fn for_project(project: &Path, diff_dir: Option<PathBuf>) -> Self {
        CompareOptions {
            project_time: std::fs::metadata(project).and_then(|m| m.modified()).ok(),
            diff_dir,
            output: OutputOptions::default(),
        }
    }
}

pub fn compare_document(
    doc: &Document,
    export_dir: &Path,
    options: &CompareOptions,
    progress: &mut dyn FnMut(&str),
) -> std::io::Result<CompareTable> {
    if let Some(dir) = &options.diff_dir {
        std::fs::create_dir_all(dir)?;
    }
    let finder = ExportFinder::scan(export_dir);
    let names: Vec<String> = doc.texture_sets.iter().map(|s| s.name.clone()).collect();
    let mut table = CompareTable::default();
    let output = match PainterNormals::of(doc) {
        Some(n) => n.apply(&options.output),
        None => {
            table.notes.push("no document.bin kept: default normal settings".into());
            options.output
        }
    };
    let options = &CompareOptions {
        output,
        project_time: options.project_time,
        diff_dir: options.diff_dir.clone(),
    };
    for set in &doc.texture_sets {
        let exports = finder.for_set(&set.name, &names);
        if exports.is_empty() {
            table.notes.push(format!("{}: no exports found", set.name));
            continue;
        }
        progress(&format!("comparing {}", set.name));
        compare_set(doc, set, &exports, options, &mut table)?;
    }
    Ok(table)
}

fn compare_set(
    doc: &Document,
    set: &TextureSet,
    exports: &[ExportMap],
    options: &CompareOptions,
    table: &mut CompareTable,
) -> std::io::Result<()> {
    let (w, h) = (set.resolution.0 as usize, set.resolution.1 as usize);
    let maps = MaterialMaps::build(set, &options.output);
    let mut mask = UvMask::rasterise(&doc.mesh, set.material_slot, set.uv_set, w, h);
    mask.shrink(BORDER);
    let mut errors = Vec::new();
    let mut loaded: Option<(PathBuf, Tex)> = None;
    for export in exports {
        let Some(ours) = maps.map(export.channel) else { continue };
        if loaded.as_ref().map(|(p, _)| p) != Some(&export.path) {
            match load_export(&export.path) {
                Ok(t) => loaded = Some((export.path.clone(), resampled(t, w, h))),
                Err(e) => {
                    table.notes.push(format!("{}: {e}", export.path.display()));
                    loaded = None;
                    continue;
                }
            }
        }
        let Some((_, theirs)) = &loaded else { continue };
        let theirs = convention::theirs_in_our_terms(theirs, export);
        let ours = &convention::ours_as_exported(export.channel, ours);
        let comps: &[usize] = if export.channel.is_color() { &[0, 1, 2] } else { &[0] };
        let diff = Diff::measure(&ours.px, &theirs.px, comps, &mask.inside, &mut errors);
        let mut scratch = Vec::new();
        let flipped_mae = Diff::measure(&ours.px, &flipped(&theirs).px, comps, &mask.inside, &mut scratch).mae;
        let map = match export.component {
            Some(c) => format!("{}[{c}]", export.suffix),
            None => export.suffix.clone(),
        };
        if let Some(dir) = &options.diff_dir {
            let name = format!("{}_{}_diff.png", file_safe(&set.name), map.replace(['[', ']'], "_"));
            heatmap::write_heatmap(&errors, &mask.inside, w, h, &dir.join(name))?;
        }
        table.rows.push(CompareRow {
            set: set.name.clone(),
            map,
            file: export
                .path
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
            newer: is_newer(&export.path, options.project_time),
            diff,
            flipped_mae,
            fit: Fit::of(&ours.px, &theirs.px, &mask.inside),
        });
    }
    Ok(())
}

fn is_newer(path: &Path, project: Option<SystemTime>) -> bool {
    let export = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    matches!((export, project), (Some(e), Some(p)) if e >= p)
}
