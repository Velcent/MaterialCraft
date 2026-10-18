//! Opening a scenario's [`Source`] the way the Paint screen does: File > New project from a mesh, loading a
//! `.mcpaint`, or importing a `.spp`; then every set composited ([`Bench::open`]).

use super::Bench;
use materialcraft::paint::Document;
use materialcraft::paint::new_project::{self, NewProject, ProjectTemplate};
use materialcraft_perf::{RunReport, Source};
use std::path::Path;
use std::time::Instant;

/// Opens `source`; puts `create_ms` or `load_ms`, `open_ms` (every set composited and on the GPU queue) with its
/// compositing and preparation times, and the document's size.
pub fn open(source: &Source, r: &mut RunReport) -> Result<Bench, String> {
    let started = Instant::now();
    let doc = match source {
        Source::NewProject { mesh, resolution } => {
            let template = ProjectTemplate::builtins().into_iter().next().ok_or("no project template")?;
            let mut settings = NewProject::new(mesh, template);
            settings.resolution = *resolution;
            let created = new_project::create(&settings, &mut |_, _| {}).map_err(|e| format!("new project: {e}"))?;
            r.put_ms("create_ms", started.elapsed());
            created.document
        }
        Source::Document(path) => {
            let doc = load(path)?;
            r.put_ms("load_ms", started.elapsed());
            doc
        }
    };
    let (b, opened) = Bench::open(doc, source.path().to_path_buf());
    let stats = b.scene.stats();
    r.put_ms("open_ms", opened);
    r.put_ms("open.composite_ms", stats.composite);
    r.put_ms("open.prepare_ms", stats.prepare);
    let d = b.doc.doc();
    r.put("sets", d.texture_sets.len() as f64);
    r.put("layers", d.texture_sets.iter().map(|s| s.walk_layers().count()).sum::<usize>() as f64);
    r.put("parts", d.mesh.parts.len() as f64);
    r.put("resolution", d.texture_sets.iter().map(|s| s.resolution.0).max().unwrap_or(0) as f64);
    Ok(b)
}

fn load(path: &Path) -> Result<Document, String> {
    let spp = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("spp"));
    if spp {
        materialcraft_substance::translate::import_spp(path, &mut |_, _| {}).map_err(|e| format!("import {}: {e}", path.display()))
    } else {
        materialcraft::paint::format::load_path(path).map_err(|e| format!("load {}: {e}", path.display()))
    }
}
