//! [`generator_cases`]: the channels of a project that measure MaterialCraft's replacement generators. Every
//! replaceable Substance procedural is replaced; a channel is a case when it is exact apart from replacements,
//! so the difference between our composite and Painter's texture cache comes from the replacements alone. Each
//! case names the generators involved (a case with one generator measures that generator).
//!
//! Used by `mcparity` (`generator.<id>` subjects): the images go into its side-by-side report.

use super::prepared::PreparedSet;
use super::project_mesh::ProjectMesh;
use crate::project::SppProject;
use crate::translate::BakedMaps;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::layers::cache_score::final_look;
use crate::translate::layers::{BlendTable, Computability, ProjectImages};
use crate::translate::replacements::{Choice, ProceduralPlan};
use materialcraft::tex::Tex;
use std::path::Path;

/// The marker the mask import writes before a replacement generator's id (`… replaced by dirt`).
const REPLACED_BY: &str = " replaced by ";

/// One channel of one set that only replacements keep from being exact.
pub struct GeneratorCase {
    pub set: String,
    pub channel: String,
    /// Colour channels compare three components, the rest one.
    pub color: bool,
    /// Our generator ids involved, sorted, without repeats.
    pub generators: Vec<String>,
    /// Our composite and Painter's cache as final looks (stored encoding), the same size.
    pub ours: Tex,
    pub theirs: Tex,
    /// Texels on the mesh (all true without a mesh).
    pub mask: Vec<bool>,
}

/// Every case of the project at `path`. `want` filters by the generators involved before anything is
/// composited (e.g. only cases of one generator).
pub fn generator_cases(path: &Path, want: &dyn Fn(&[String]) -> bool) -> Result<Vec<GeneratorCase>, String> {
    let project = SppProject::open(path).map_err(|e| e.to_string())?;
    let (document, _) = project.model().map_err(|e| e.to_string())?;
    let cache = CacheChannels::new(&project);
    let images = ProjectImages::new(&project);
    let mesh = ProjectMesh::read(&project);
    let baked = BakedMaps::new(&project);
    let records = project.document_materials().unwrap_or_default();
    let table = BlendTable::painter();
    let mut out = Vec::new();
    for painter in &document.texture_sets {
        let Some(mut set) = PreparedSet::new(painter, &cache, mesh.as_ref()) else {
            continue;
        };
        set.plan = ProceduralPlan::all(Choice::Replace);
        let stack = set.import(&table, &images);
        let candidates: Vec<_> = stack
            .verdict
            .channels()
            .into_iter()
            .filter_map(|(channel, status)| match &status {
                Computability::Replaced(reasons) => Some((channel, generators_of(reasons))),
                _ => None,
            })
            .filter(|(_, g)| !g.is_empty() && want(g))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        set.load_baked(&baked, &records);
        set.plan = ProceduralPlan::default();
        let kept = set.import(&table, &images);
        for (channel, generators) in candidates {
            if !set.load(channel, &cache) {
                continue;
            }
            let (Some(image), Some(spec)) = (set.cache(channel), set.channels.spec(channel)) else {
                continue;
            };
            let theirs = final_look(image, spec);
            let ours = set.composite(&stack, channel);
            // Channels the replaced generators do not change (the masked layer writes the same value as
            // what is below it) measure nothing: leave them out.
            let without = set.composite(&kept, channel);
            if !differs(&ours, &without) {
                continue;
            }
            let mask = (0..theirs.w * theirs.h)
                .map(|i| set.islands().is_none_or(|m| m.contains(i % theirs.w, i / theirs.w)))
                .collect();
            out.push(GeneratorCase {
                set: painter.name.clone(),
                channel: set.channels.label(channel),
                color: channel.is_color(),
                generators,
                ours,
                theirs,
                mask,
            });
        }
    }
    Ok(out)
}

/// True when two composites differ by at least one 8-bit step somewhere.
fn differs(a: &Tex, b: &Tex) -> bool {
    a.px.iter()
        .zip(&b.px)
        .any(|(x, y)| (0..3).any(|i| (x[i] - y[i]).abs() >= 1.0 / 255.0))
}

/// The generator ids named in a channel's replacement reasons.
fn generators_of(reasons: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = reasons
        .iter()
        .filter_map(|r| r.rsplit_once(REPLACED_BY).map(|(_, id)| id.to_string()))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_ids_come_from_the_reasons() {
        let reasons = vec![
            "mask generator \"mg_dirt\" replaced by dirt".to_string(),
            "mask generator \"mg_ambient_occlusion\" replaced by ambient_occlusion".to_string(),
            "mask generator \"mg_dirt\" replaced by dirt".to_string(),
        ];
        assert_eq!(generators_of(&reasons), ["ambient_occlusion", "dirt"]);
        assert!(generators_of(&["other".to_string()]).is_empty());
    }
}
