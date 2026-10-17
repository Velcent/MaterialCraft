//! [`survey`]: which projects and texture sets one subject could run on, and why the others cannot.
//! Opens each project and resolves the inputs' locations, but loads no high-poly mesh and bakes nothing.

use crate::project::OpenedProject;
use crate::reference::References;
use crate::subject::Subject;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct SurveyRow {
    pub project: PathBuf,
    /// Empty when the project itself could not be opened.
    pub set: String,
    /// `Ok(where the high mesh comes from)` or `Err(why the case cannot run)`.
    pub status: Result<String, String>,
}

pub fn survey(subject: &dyn Subject, projects: &[PathBuf], progress: &mut dyn FnMut(&str)) -> Vec<SurveyRow> {
    let mut rows = Vec::new();
    for path in projects {
        progress(&path.display().to_string());
        let project = match OpenedProject::open(path) {
            Ok(p) => p,
            Err(e) => {
                rows.push(SurveyRow {
                    project: path.clone(),
                    set: String::new(),
                    status: Err(format!("cannot open: {e}")),
                });
                continue;
            }
        };
        let references = References::new(&project);
        for set in &project.sets {
            let status = if !references.has(set, &subject.reference()) {
                Err(format!("Painter stored no {}", subject.reference()))
            } else {
                project
                    .high_source(set)
                    .map(|s| s.label())
                    .map_err(|e| e.reason().to_string())
            };
            rows.push(SurveyRow {
                project: path.clone(),
                set: set.name.clone(),
                status,
            });
        }
    }
    rows
}

/// Reasons and how often each occurs (most frequent first), with the file names of high-poly meshes
/// folded into one reason per kind.
pub fn reason_counts(rows: &[SurveyRow]) -> Vec<(String, usize)> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for reason in rows.iter().filter_map(|r| r.status.as_ref().err()) {
        let key = generalise(reason);
        match counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts
}

/// `high-poly 'X.obj' not found …` → `high-poly '…' not found …`.
pub fn generalise(reason: &str) -> String {
    match (reason.find('\''), reason.rfind('\'')) {
        (Some(a), Some(b)) if b > a => format!("{}'…'{}", &reason[..a], &reason[b + 1..]),
        _ => reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_generalised_reasons() {
        let row = |status: Result<&str, &str>| SurveyRow {
            project: PathBuf::from("p.spp"),
            set: "s".into(),
            status: status.map(String::from).map_err(String::from),
        };
        let rows = [
            row(Err("high-poly 'a.obj' not found below the project folder")),
            row(Err("high-poly 'b.obj' not found below the project folder")),
            row(Err("no high-poly mesh recorded")),
            row(Ok("low as high")),
        ];
        let counts = reason_counts(&rows);
        assert_eq!(
            counts[0],
            ("high-poly '…' not found below the project folder".to_string(), 2)
        );
        assert_eq!(counts.len(), 2);
    }
}
