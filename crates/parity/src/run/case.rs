//! What one case produced: [`CaseResult`] (kept for tables and history) and [`CaseImages`] (handed to the
//! report once, then dropped, so a long run does not keep every image in memory).

use crate::metrics::Metrics;
use materialcraft::tex::Tex;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Measured(Box<Measured>),
    Skipped(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Measured {
    pub metrics: Metrics,
    /// Compared size.
    pub size: (usize, usize),
    /// Painter's map name and stored size.
    pub reference: String,
    pub reference_size: (usize, usize),
    /// Where the high mesh came from.
    pub high: String,
    pub notes: Vec<String>,
    pub unmapped: Vec<String>,
    pub settings_hash: String,
    /// Seconds spent on our side (the shared bake counts for the first subject of a set).
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaseResult {
    pub subject: &'static str,
    pub project: PathBuf,
    pub project_name: String,
    pub set: String,
    pub outcome: Outcome,
}

impl CaseResult {
    pub fn measured(&self) -> Option<&Measured> {
        match &self.outcome {
            Outcome::Measured(m) => Some(m),
            _ => None,
        }
    }

    /// `Project / set`.
    pub fn label(&self) -> String {
        format!("{} / {}", self.project_name, self.set)
    }
}

/// The images of a measured case, at the compared size.
pub struct CaseImages<'a> {
    pub ours: &'a Tex,
    pub theirs: &'a Tex,
    pub mask: &'a [bool],
    pub errors: &'a [f32],
}

/// Receives each case as soon as it is done.
pub trait CaseSink {
    fn case(&mut self, result: &CaseResult, images: Option<&CaseImages>);
    /// A progress or problem message (not a case).
    fn note(&mut self, _message: &str) {}
}
