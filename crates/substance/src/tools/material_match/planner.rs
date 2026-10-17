//! [`ProjectPlanner`]: the plan a batch tool imports each project with: the same [`ProceduralPlan`] for every
//! project, optionally with each project's material fills replaced by their best-scored tray material
//! (`mcsub … --materials auto`), and the library the picked materials are embedded from.

use super::MaterialMatches;
use super::tray::TrayMaterials;
use crate::tools::accuracy::ReplacementAccuracy;
use crate::tools::analyze::Analysis;
use crate::translate::replacements::ProceduralPlan;
use materialcraft::material_fill::MaterialResolver;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct ProjectPlanner {
    base: ProceduralPlan,
    /// The tray materials, when there is a library.
    tray: Option<Arc<TrayMaterials>>,
    /// Replace every material fill the base plan does not choose by its best suggestion.
    auto: bool,
}

impl ProjectPlanner {
    /// Every project with `plan`.
    pub fn fixed(plan: ProceduralPlan) -> ProjectPlanner {
        ProjectPlanner {
            base: plan,
            tray: None,
            auto: false,
        }
    }

    /// Builder: the library's materials; with `auto`, every material fill not chosen in the plan gets its best
    /// suggestion.
    pub fn with_tray(mut self, tray: Arc<TrayMaterials>, auto: bool) -> ProjectPlanner {
        self.tray = Some(tray);
        self.auto = auto;
        self
    }

    pub fn base(&self) -> &ProceduralPlan {
        &self.base
    }

    pub fn tray(&self) -> Option<&TrayMaterials> {
        self.tray.as_deref()
    }

    /// Where picked materials are read from to embed them.
    pub fn library(&self) -> Option<Arc<dyn MaterialResolver>> {
        self.tray
            .as_ref()
            .map(|t| Arc::new(t.resolver()) as Arc<dyn MaterialResolver>)
    }

    /// The plan for the project at `path` and, with `auto`, the matches it was chosen from.
    pub fn plan_for(&self, path: &Path) -> Result<(ProceduralPlan, Option<MaterialMatches>), String> {
        let Some(tray) = self.tray.as_deref().filter(|_| self.auto) else {
            return Ok((self.base.clone(), None));
        };
        let analysis = Analysis::of_project(path, &ReplacementAccuracy::default())?;
        let matches = MaterialMatches::measure(path, &analysis, tray)?;
        Ok((matches.best_plan(self.base.clone()), Some(matches)))
    }
}
