//! Instance checks (IN-08, IN-11): links that name no layer, instances that depend on themselves, and channels a
//! source uses that the instance's texture set does not declare. These are reported, not refused: the instance is
//! broken or incomplete, and the document still opens and saves.

use crate::paint::channel::Channel;
use crate::paint::doc::Document;
use crate::paint::instance::{Broken, InstanceIndex, InstanceLink};
use crate::paint::texture_set::TextureSet;

/// What is wrong with an instance.
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceProblem {
    /// The linked texture set or layer does not exist: the instance composites nothing.
    MissingSource,
    /// The instance depends on itself (it sits inside the folder it instances, directly or through other instances):
    /// it composites nothing until the cycle is undone. Not repaired automatically.
    Cycle,
    /// The source uses channels the instance's set does not declare: those parts of it are left out.
    MissingChannels(Vec<Channel>),
}

/// A problem of instance layer `layer` in texture set `set` (ids).
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceIssue {
    pub set: String,
    pub layer: String,
    pub problem: InstanceProblem,
}

/// Every instance problem of `doc`, in document order.
pub fn instance_issues(doc: &Document) -> Vec<InstanceIssue> {
    let mut out = Vec::new();
    for e in InstanceIndex::of(doc).entries {
        let problem = match e.broken {
            Some(Broken::Missing) => InstanceProblem::MissingSource,
            Some(Broken::Cycle) => InstanceProblem::Cycle,
            None => {
                let Some(set) = doc.texture_sets.get(e.set_index) else { continue };
                let missing = missing_channels(doc, &e.link, set);
                if missing.is_empty() {
                    continue;
                }
                InstanceProblem::MissingChannels(missing)
            }
        };
        out.push(InstanceIssue { set: e.set, layer: e.layer, problem });
    }
    out
}

/// The channels the layers an instance of `link` reads blend into (layer and content effect entries) that `target`
/// does not declare, in channel order (IN-08: the instantiate dialog marks such sets).
pub fn missing_channels(doc: &Document, link: &InstanceLink, target: &TextureSet) -> Vec<Channel> {
    let mut out: Vec<Channel> = Vec::new();
    for l in doc.instance_reads(link) {
        let entries = l.blend.iter().chain(l.effects.iter().flat_map(|e| e.blend.iter()));
        for b in entries {
            if !target.has_channel(b.channel) && !out.contains(&b.channel) {
                out.push(b.channel);
            }
        }
    }
    out.sort_by_key(|c| c.id());
    out
}
