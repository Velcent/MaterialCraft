//! Duplicating every layer of real imported projects (the layer panel's Ctrl+D and Ctrl+drag): each copy
//! is made, the document stays valid, and undoing the copy gives back the identical document. Skipped unless
//! `MC_DUPLICATE_CORPUS` names `.spp` files (separated by the platform's path separator) or a text file
//! listing one per line. Nothing read from those files is kept in the repository.

use materialcraft::paint::Document;
use materialcraft::paint::edit::ops;
use materialcraft::paint::edit::{Edit, LayerSlot, Transaction};
use materialcraft_substance::translate::import_spp;
use std::path::{Path, PathBuf};

fn projects() -> Option<Vec<PathBuf>> {
    let var = std::env::var_os("MC_DUPLICATE_CORPUS")?;
    let mut out = Vec::new();
    for p in std::env::split_paths(&var) {
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")) {
            let list = std::fs::read_to_string(&p).expect("project list");
            out.extend(list.lines().map(str::trim).filter(|l| !l.is_empty()).map(PathBuf::from));
        } else {
            out.push(p);
        }
    }
    Some(out)
}

/// Undoes `inverses` (as the history does: last first).
fn undo(doc: &mut Document, inverses: Vec<Edit>) {
    for e in inverses.into_iter().rev() {
        e.apply(doc).expect("undo");
    }
}

fn check_project(path: &Path, failures: &mut Vec<String>) -> usize {
    let original = match import_spp(path, &mut |_, _| {}) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: not imported ({e}); skipped", path.display());
            return 0;
        }
    };
    let valid_before = original.problems().is_empty();
    let mut doc = original.clone();
    let mut tried = 0;
    let sets: Vec<(String, Vec<String>)> = original
        .texture_sets
        .iter()
        .map(|s| (s.id.clone(), s.walk_layers().map(|(_, l)| l.id.clone()).collect()))
        .collect();
    for (set, ids) in sets {
        for id in ids {
            tried += 1;
            let what = format!("{} / {set} / {id}", path.display());
            for to_slot in [false, true] {
                let mut tx = Transaction::new(&mut doc);
                let made = if to_slot {
                    ops::duplicate_to(&mut tx, &set, std::slice::from_ref(&id), LayerSlot::top(0))
                } else {
                    ops::duplicate(&mut tx, &set, std::slice::from_ref(&id))
                };
                match made {
                    Ok(copies) if copies.len() == 1 => {
                        if valid_before && let Some(p) = tx.doc().problems().first() {
                            failures.push(format!("{what}: the copy breaks the document: {p}"));
                        }
                        let inverses = tx.commit();
                        undo(&mut doc, inverses);
                        if doc != original {
                            failures.push(format!("{what}: undoing the copy does not give the original back"));
                            doc = original.clone();
                        }
                    }
                    Ok(copies) => failures.push(format!("{what}: {} copies", copies.len())),
                    Err(e) => failures.push(format!("{what}: not copied: {e}")),
                }
            }
        }
    }
    tried
}

#[test]
fn every_layer_of_real_projects_duplicates_and_undoes() {
    let Some(files) = projects() else { return };
    let mut failures = Vec::new();
    for path in &files {
        let n = check_project(path, &mut failures);
        eprintln!("{}: {n} layers duplicated", path.display());
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
