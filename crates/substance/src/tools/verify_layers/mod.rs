//! `mcsub verify-layers`: imports each texture set's layer stack, composites every channel the stack can
//! compute and compares it with Painter's texture cache (the flattened result Painter saved), per set and
//! channel, on the texels of the mesh. With `--fit`, guessed blend values are identified by trying each of
//! our modes ([`fit`]).

mod fit;
mod prepared;
mod project_mesh;
mod table;

pub use crate::translate::layers::cache_score::{EXACT_STEPS, Score};
pub use fit::{FitRecord, FitTally};
pub use table::Verification;

use crate::inspect::Corpus;
use crate::project::SppProject;
use crate::tools::parallel::CorpusPool;
use crate::translate::BakedMaps;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::layers::cache_agreement::{CacheAgreement, disagrees};
use crate::translate::layers::{BlendTable, Computability, ProjectImages, StackStrategy};
use crate::translate::replacements::ProceduralPlan;
use materialcraft::paint::Channel;
use prepared::PreparedSet;
use project_mesh::ProjectMesh;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// Also fit guessed blend values.
    pub fit: bool,
    /// Which Substance procedurals are replaced by MaterialCraft's generators before comparing.
    pub plan: ProceduralPlan,
}

/// One channel of one set.
pub struct ChannelResult {
    pub channel: Channel,
    pub label: String,
    pub status: Computability,
    /// `None` when the channel is not computable or has no cache image.
    pub score: Option<Score>,
}

/// One texture set with a texture cache.
pub struct SetResult {
    pub project: String,
    pub set: String,
    pub strategy: StackStrategy,
    pub channels: Vec<ChannelResult>,
}

/// What one project contributed, merged into the [`Verification`] in project order.
#[derive(Default)]
struct ProjectVerification {
    sets: Vec<SetResult>,
    fits: Vec<FitRecord>,
}

/// Verifies every `.spp` below `root` (or `root` itself when it is a file), several projects at once on `pool`.
/// The result is the same as one project after another.
pub fn verify_path(
    root: &Path,
    options: VerifyOptions,
    pool: CorpusPool,
    progress: &(dyn Fn(&Path) + Sync),
) -> Verification {
    let files = match root.is_file() {
        true => vec![root.to_path_buf()],
        false => Corpus::discover(root).files().to_vec(),
    };
    let label_of = |path: &PathBuf| {
        let relative = path.strip_prefix(root).ok().filter(|p| !p.as_os_str().is_empty());
        relative
            .or_else(|| path.file_name().map(Path::new))
            .unwrap_or(path)
            .display()
            .to_string()
    };
    let results = pool.map(
        &files,
        |path| CorpusPool::estimate(path),
        |path| {
            progress(path);
            verify_project(path, &label_of(path), &options)
        },
    );
    let mut out = Verification::default();
    for (path, result) in files.iter().zip(results) {
        match result {
            Ok(project) => {
                out.sets.extend(project.sets);
                out.fits.add(project.fits);
            }
            Err(e) => out.failures.push((label_of(path), e)),
        }
    }
    out
}

fn verify_project(path: &Path, label: &str, options: &VerifyOptions) -> Result<ProjectVerification, String> {
    let mut out = ProjectVerification::default();
    let project = SppProject::open(path).map_err(|e| e.to_string())?;
    let (document, _) = project.model().map_err(|e| e.to_string())?;
    let cache = CacheChannels::new(&project);
    let images = ProjectImages::new(&project);
    let mesh = ProjectMesh::read(&project);
    let table = BlendTable::painter();
    let baked = BakedMaps::new(&project);
    let records = project.document_materials().unwrap_or_default();
    for painter in &document.texture_sets {
        let Some(mut set) = PreparedSet::new(painter, &cache, mesh.as_ref()) else {
            continue;
        };
        set.plan = options.plan.clone();
        if options.plan != ProceduralPlan::default() {
            let refs = records
                .iter()
                .find(|r| r.name == painter.name)
                .map(|r| r.baked.clone())
                .unwrap_or_default();
            set.baked = baked
                .sources(&painter.name, &refs)
                .iter()
                .filter_map(|s| baked.read(s).ok().map(|i| i.map))
                .collect();
        }
        let stack = set.import(&table, &images);
        let mut channels = Vec::new();
        for (channel, status) in stack.verdict.channels() {
            let compared = status.is_computable() && set.load(channel, &cache);
            let score = if compared { set.score(&stack, channel) } else { None };
            if let (Some(score), true, Some(values)) =
                (score, options.fit, stack.verdict.blend_values.get(&channel.id()))
            {
                let example = format!("{label} / {} / {}", painter.name, set.channels.label(channel));
                out.fits.extend(FitTally::measure(
                    &set,
                    channel,
                    values,
                    score,
                    (&table, &images),
                    &example,
                ));
            }
            channels.push(ChannelResult {
                channel,
                label: set.channels.label(channel),
                status,
                score,
            });
        }
        // The importer's decision: a fully computable stack must also agree with the cache.
        let mut agreement = CacheAgreement::default();
        if stack.verdict.all_exact() {
            let off = channels
                .iter()
                .filter_map(|c| c.score.filter(disagrees).map(|s| (c.channel, s)));
            agreement.disagreeing = off.collect();
        }
        let strategy = StackStrategy::choose(&stack.verdict, true, &agreement);
        out.sets.push(SetResult {
            project: label.to_string(),
            set: painter.name.clone(),
            strategy,
            channels,
        });
    }
    Ok(out)
}
