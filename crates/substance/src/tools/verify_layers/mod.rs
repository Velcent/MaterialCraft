//! `mcsub verify-layers`: imports each texture set's layer stack, composites every channel the stack can
//! compute and compares it with Painter's texture cache (the flattened result Painter saved), per set and
//! channel. With `--fit`, guessed blend values are identified by trying each of our modes ([`fit`]).

mod fit;
mod prepared;
mod table;

pub use crate::translate::layers::cache_score::{EXACT_STEPS, Score};
pub use fit::FitTally;
pub use table::Verification;

use crate::inspect::Corpus;
use crate::project::SppProject;
use crate::translate::cache_channels::CacheChannels;
use crate::translate::layers::cache_agreement::{CacheAgreement, disagrees};
use crate::translate::layers::{BlendTable, Computability, ProjectImages, StackStrategy};
use materialcraft::paint::Channel;
use prepared::PreparedSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default)]
pub struct VerifyOptions {
    /// Also fit guessed blend values.
    pub fit: bool,
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

/// Verifies every `.spp` below `root` (or `root` itself when it is a file).
pub fn verify_path(root: &Path, options: VerifyOptions, progress: &mut dyn FnMut(&Path)) -> Verification {
    let mut out = Verification::default();
    let files = match root.is_file() {
        true => vec![root.to_path_buf()],
        false => Corpus::discover(root).files().to_vec(),
    };
    for path in &files {
        progress(path);
        let relative = path.strip_prefix(root).ok().filter(|p| !p.as_os_str().is_empty());
        let label = relative
            .or_else(|| path.file_name().map(Path::new))
            .unwrap_or(path)
            .display()
            .to_string();
        if let Err(e) = verify_project(path, &label, options, &mut out) {
            out.failures.push((label, e));
        }
    }
    out
}

fn verify_project(path: &Path, label: &str, options: VerifyOptions, out: &mut Verification) -> Result<(), String> {
    let project = SppProject::open(path).map_err(|e| e.to_string())?;
    let (document, _) = project.model().map_err(|e| e.to_string())?;
    let cache = CacheChannels::new(&project);
    let images = ProjectImages::new(&project);
    let table = BlendTable::painter();
    for painter in &document.texture_sets {
        let Some(mut set) = PreparedSet::new(painter, &cache) else {
            continue;
        };
        let stack = set.import(&table, &images);
        let mut channels = Vec::new();
        for (channel, status) in stack.verdict.channels() {
            let compared = status.is_computable() && set.load(channel, &cache);
            let score = if compared { set.score(&stack, channel) } else { None };
            if let (Some(score), true, Some(values)) =
                (score, options.fit, stack.verdict.blend_values.get(&channel.id()))
            {
                let example = format!("{label} / {} / {}", painter.name, set.channels.label(channel));
                out.fits.fit(&set, channel, values, score, (&table, &images), &example);
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
    Ok(())
}
