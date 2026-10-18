//! The project-level entry points: [`project_plans`] lists every candidate case of a project with its status
//! (imports only, nothing composited), [`isolated_cases`] measures the ones a caller wants and hands each to a
//! sink as soon as it is done.

use super::analysis::SetAnalysis;
use super::cached_images::CachedImages;
use super::frames::FrameGrid;
use super::inverse::InversionSettings;
use super::joint_measure::MultiComposer;
use super::measure::{Composer, Measurement, Measurer};
use super::plan::{CasePlan, plans};
use crate::model::PainterTextureSet;
use crate::project::SppProject;
use crate::tools::verify_layers::prepared::PreparedSet;
use crate::tools::verify_layers::project_mesh::ProjectMesh;
use crate::translate::BakedMaps;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::layers::cache_score::final_look;
use crate::translate::layers::{BlendTable, Computability, ImageSource, ProjectImages};
use crate::translate::replacements::ProceduralPlan;
use materialcraft::paint::Channel;
use materialcraft::tex::Tex;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct IsolateOptions {
    /// Longest side of the compared images.
    pub max_size: usize,
    /// Composite our replacement too (cases of candidates we replace).
    pub with_ours: bool,
    pub settings: InversionSettings,
}

impl Default for IsolateOptions {
    fn default() -> Self {
        IsolateOptions {
            max_size: 1024,
            with_ours: true,
            settings: InversionSettings::default(),
        }
    }
}

/// The candidate cases of one texture set.
pub struct SetPlans {
    pub set: String,
    pub plans: Vec<CasePlan>,
}

/// One measured case.
pub struct IsolatedCase {
    pub set: String,
    /// Channel label as Painter names it.
    pub channel: String,
    pub plan: CasePlan,
    pub measurement: Measurement,
}

/// Every candidate case of the project at `path`, set by set (sets without a texture cache are left out).
pub fn project_plans(path: &Path) -> Result<Vec<SetPlans>, String> {
    let project = SppProject::open(path).map_err(|e| e.to_string())?;
    let (document, _) = project.model().map_err(|e| e.to_string())?;
    let cache = CacheChannels::new(&project);
    let raw = ProjectImages::new(&project);
    let images = CachedImages::new(&raw);
    let table = BlendTable::painter();
    let mesh = ProjectMesh::read(&project);
    let mut out = Vec::new();
    for painter in &document.texture_sets {
        let Some(set) = PreparedSet::new(painter, &cache, mesh.as_ref()) else {
            continue;
        };
        out.push(SetPlans {
            set: painter.name.clone(),
            plans: plans(&analyse(&set, painter, &table, &images)),
        });
    }
    Ok(out)
}

/// Measures every measurable case `want` accepts and hands it to `sink`.
pub fn isolated_cases(
    path: &Path,
    want: &dyn Fn(&CasePlan) -> bool,
    options: &IsolateOptions,
    sink: &mut dyn FnMut(IsolatedCase),
) -> Result<(), String> {
    let project = SppProject::open(path).map_err(|e| e.to_string())?;
    let (document, _) = project.model().map_err(|e| e.to_string())?;
    let cache = CacheChannels::new(&project);
    let raw = ProjectImages::new(&project);
    let images = CachedImages::new(&raw);
    let table = BlendTable::painter();
    let mesh = ProjectMesh::read(&project);
    let baked = BakedMaps::new(&project);
    let records = project.document_materials().unwrap_or_default();
    for painter in &document.texture_sets {
        let Some(mut set) = PreparedSet::new(painter, &cache, mesh.as_ref()) else {
            continue;
        };
        let wanted: Vec<CasePlan> = plans(&analyse(&set, painter, &table, &images))
            .into_iter()
            .filter(|p| p.status.measurable() && want(p))
            .collect();
        if wanted.is_empty() {
            continue;
        }
        if options.with_ours {
            set.load_baked(&baked, &records);
        }
        for plan in &wanted {
            if !set.load(plan.channel, &cache) {
                continue;
            }
            let (Some(image), Some(spec)) = (set.cache(plan.channel), set.channels.spec(plan.channel)) else {
                continue;
            };
            let look = final_look(image, spec);
            let grid = FrameGrid::new((look.w, look.h), options.max_size, set.islands());
            let observed = grid.frame(&look);
            drop(look);
            let composer = StackComposer {
                set: &set,
                table: &table,
                images: &images,
            };
            let measurer = Measurer {
                composer: &composer,
                painter,
                channels: &set.channels,
                grid: &grid,
                settings: options.settings,
            };
            sink(IsolatedCase {
                set: painter.name.clone(),
                channel: set.channels.label(plan.channel),
                plan: plan.clone(),
                measurement: measurer.measure(plan, &observed, options.with_ours),
            });
        }
    }
    Ok(())
}

/// The set's analysis, importing with `set`'s plan (keep every procedural) on the set's mesh.
pub(super) fn analyse(
    set: &PreparedSet,
    painter: &PainterTextureSet,
    table: &BlendTable,
    images: &dyn ImageSource,
) -> SetAnalysis {
    let import = |s: &PainterTextureSet| set.import_copy(s, table, images, &set.plan).verdict;
    SetAnalysis::new(painter, &set.channels, &import)
}

/// Imports with the set's channels, resolution and mesh and composites as `verify-layers` does.
pub(super) struct StackComposer<'s, 'a> {
    pub set: &'s PreparedSet<'a>,
    pub table: &'s BlendTable,
    pub images: &'s dyn ImageSource,
}

impl Composer for StackComposer<'_, '_> {
    fn composite(
        &self,
        painter: &PainterTextureSet,
        procedurals: &ProceduralPlan,
        channel: Channel,
    ) -> (Tex, Computability) {
        let stack = self.set.import_copy(painter, self.table, self.images, procedurals);
        (self.set.composite(&stack, channel), stack.verdict.channel(channel))
    }
}

impl MultiComposer for StackComposer<'_, '_> {
    fn composite_channels(
        &self,
        painter: &PainterTextureSet,
        procedurals: &ProceduralPlan,
        channels: &[Channel],
    ) -> Vec<(Tex, Computability)> {
        let stack = self.set.import_copy(painter, self.table, self.images, procedurals);
        channels
            .iter()
            .map(|c| (self.set.composite(&stack, *c), stack.verdict.channel(*c)))
            .collect()
    }
}
