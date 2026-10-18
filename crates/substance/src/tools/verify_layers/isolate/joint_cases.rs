//! Project-level entry points of the joint read-back: [`joint_project_plans`] lists every candidate mask's
//! [`JointPlan`] (imports only), [`joint_cases`] measures the feasible ones a caller wants.

use super::cached_images::CachedImages;
use super::cases::{StackComposer, analyse};
use super::frames::FrameGrid;
use super::joint_measure::JointMeasurer;
use super::joint_plan::{JointPlan, JointStatus, joint_plans};
use super::joint_solve::JointSettings;
use super::measure::Measurement;
use super::plan::plans;
use crate::project::SppProject;
use crate::tools::verify_layers::prepared::PreparedSet;
use crate::tools::verify_layers::project_mesh::ProjectMesh;
use crate::translate::BakedMaps;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::layers::cache_score::final_look;
use crate::translate::layers::{BlendTable, ProjectImages};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct JointOptions {
    /// Longest side of the compared images (the probe grid keeps 3^k composites per channel in memory).
    pub max_size: usize,
    pub with_ours: bool,
    pub settings: JointSettings,
}

impl Default for JointOptions {
    fn default() -> Self {
        JointOptions {
            max_size: 512,
            with_ours: true,
            settings: JointSettings::default(),
        }
    }
}

pub struct SetJointPlans {
    pub set: String,
    pub plans: Vec<JointPlan>,
}

pub struct JointCase {
    pub set: String,
    /// The observed channels, as Painter names them, joined with `+`.
    pub channels: String,
    pub plan: JointPlan,
    pub measurement: Measurement,
}

/// Every candidate mask's joint plan, set by set.
pub fn joint_project_plans(path: &Path) -> Result<Vec<SetJointPlans>, String> {
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
        out.push(SetJointPlans {
            set: painter.name.clone(),
            plans: joint_plans(&plans(&analyse(&set, painter, &table, &images))),
        });
    }
    Ok(out)
}

/// Measures every feasible joint plan `want` accepts and hands it to `sink`.
pub fn joint_cases(
    path: &Path,
    want: &dyn Fn(&JointPlan) -> bool,
    options: &JointOptions,
    sink: &mut dyn FnMut(JointCase),
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
        let wanted: Vec<JointPlan> = joint_plans(&plans(&analyse(&set, painter, &table, &images)))
            .into_iter()
            .filter(|p| p.status == JointStatus::Feasible && want(p))
            .collect();
        if wanted.is_empty() {
            continue;
        }
        if options.with_ours {
            set.load_baked(&baked, &records);
        }
        for plan in &wanted {
            let mut plan = plan.clone();
            plan.channels.retain(|c| set.load(*c, &cache));
            if plan.components() < plan.unknowns() {
                continue;
            }
            let mut looks = Vec::new();
            for c in &plan.channels {
                if let (Some(image), Some(spec)) = (set.cache(*c), set.channels.spec(*c)) {
                    looks.push(final_look(image, spec));
                }
            }
            let Some(first) = looks.first() else { continue };
            if looks.len() != plan.channels.len() || looks.iter().any(|l| (l.w, l.h) != (first.w, first.h)) {
                continue;
            }
            let grid = FrameGrid::new((first.w, first.h), options.max_size, set.islands());
            let observed: Vec<_> = looks.iter().map(|l| grid.frame(l)).collect();
            drop(looks);
            let composer = StackComposer {
                set: &set,
                table: &table,
                images: &images,
            };
            let measurer = JointMeasurer {
                composer: &composer,
                painter,
                channels: &set.channels,
                grid: &grid,
                settings: options.settings,
            };
            let measurement = measurer.measure(&plan, &observed, options.with_ours);
            let channels = plan
                .channels
                .iter()
                .map(|c| set.channels.label(*c))
                .collect::<Vec<_>>()
                .join("+");
            sink(JointCase {
                set: painter.name.clone(),
                channels,
                plan,
                measurement,
            });
        }
    }
    Ok(())
}
