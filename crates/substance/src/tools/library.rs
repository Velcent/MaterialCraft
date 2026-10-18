//! `mcsub library <file.spp> [<library dir>]`: what the import review offers for the user's library (one line per
//! candidate: kind, uses, shareable or not, origin, name), and with a folder every candidate written into it (to check
//! the export; the folder is the user's own, nothing from it is ever committed).

use crate::project::SppProject;
use crate::translate::layers::images::ProjectImages;
use crate::translate::library::{LibraryCandidates, LibraryExport};
use std::fmt::Write as _;
use std::path::Path;

/// The candidate table, and the written files when `out` is given.
pub fn run(path: &Path, out: Option<&Path>) -> Result<String, String> {
    let project = SppProject::open(path).map_err(|e| e.to_string())?;
    let candidates = LibraryCandidates::of_project(&project)?;
    let mut text = String::new();
    for c in &candidates.items {
        let share = if c.shareable() { "shareable" } else { "not shareable" };
        let _ = writeln!(
            text,
            "{:<12} {:>4}  {:<14} {:<40} {}",
            c.kind.label(),
            c.uses,
            share,
            c.origin.describe(),
            c.name
        );
    }
    for why in &candidates.left_out {
        let _ = writeln!(text, "left out: {why}");
    }
    if let Some(dir) = out {
        let keys: Vec<String> = candidates.items.iter().map(|c| c.key.clone()).collect();
        let images = ProjectImages::new(&project);
        for w in LibraryExport::new(&candidates, &images).write(&keys, dir) {
            let _ = match w.result {
                Ok(saved) => writeln!(text, "wrote {}", saved.reference),
                Err(e) => writeln!(text, "not written: {}: {e}", w.name),
            };
        }
    }
    Ok(text)
}
