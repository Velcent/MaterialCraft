//! The importer on real Painter projects. Skipped unless `MC_SUBSTANCE_CORPUS` names a folder (searched
//! recursively for `.spp` files). Nothing read from those files is kept in the repository.

use materialcraft::paint::format;
use materialcraft_substance::inspect::Corpus;
use materialcraft_substance::project::SppProject;
use materialcraft_substance::translate::{SetBinding, import_spp};
use std::path::PathBuf;

/// Full imports are slow; only this many of the smallest projects are imported completely.
const FULL_IMPORTS: usize = 3;

fn corpus() -> Option<Vec<PathBuf>> {
    let root = std::env::var_os("MC_SUBSTANCE_CORPUS")?;
    Some(Corpus::discover(std::path::Path::new(&root)).files().to_vec())
}

#[test]
fn every_mesh_material_finds_its_document_record() {
    let Some(files) = corpus() else { return };
    let mut failures = Vec::new();
    for path in &files {
        let project = SppProject::open(path).expect("open");
        let materials = project.mesh().expect("mesh").materials;
        let records = project.document_materials().expect("document");
        let binding = SetBinding::bind(&materials, &records, &Default::default());
        if binding.sets.iter().any(|s| !s.in_document) {
            failures.push(path.display().to_string());
        }
    }
    assert!(failures.is_empty(), "unbound texture sets in {failures:#?}");
}

#[test]
fn smallest_projects_import_into_valid_documents_that_save_and_load() {
    let Some(mut files) = corpus() else { return };
    files.sort_by_key(|p| std::fs::metadata(p).map_or(u64::MAX, |m| m.len()));
    for path in files.iter().take(FULL_IMPORTS) {
        let doc = import_spp(path, &mut |_, _| {}).expect("import");
        assert_eq!(doc.problems(), [], "{}", path.display());
        assert!(!doc.texture_sets.is_empty(), "{}", path.display());
        assert!(doc.import_report.is_some());
        let mut bytes = Vec::new();
        format::save(&doc, &mut bytes).expect("save");
        let loaded = format::load(std::io::Cursor::new(bytes)).expect("load");
        assert!(loaded == doc, "{} changed in a save/load round trip", path.display());
    }
}
