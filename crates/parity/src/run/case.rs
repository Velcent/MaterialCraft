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
    /// Why the reference is stale (another UV layout or high-poly source than today's files), if it is: the case
    /// is then reported as skipped and kept out of the summaries, numbers and images kept for looking at.
    pub stale: Option<String>,
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

    /// The numbers of a measured case whose reference is not stale: what tables and summaries count.
    pub fn counted(&self) -> Option<&Measured> {
        self.measured().filter(|m| m.stale.is_none())
    }

    /// Why a measured case's reference is stale, if it is.
    pub fn stale(&self) -> Option<&str> {
        self.measured().and_then(|m| m.stale.as_deref())
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
