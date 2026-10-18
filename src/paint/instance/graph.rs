//! The instance dependency graph. Its nodes are layers (texture set id, layer id); resolving a layer reads its source
//! if it is an instance and its children if it is a folder. An instance on a cycle of that graph reads itself.

use super::InstanceLink;
use crate::paint::doc::Document;
use crate::paint::layer::{Layer, LayerKind};
use std::collections::HashSet;

/// A layer of a document: (texture set id, layer id).
pub(crate) type Node<'a> = (&'a str, &'a str);

/// What a layer is to instancing (IN-09): the layer panel draws a distinct icon for each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceRole {
    /// Neither instanced nor an instance.
    Normal,
    /// Other layers are instances of it.
    Source,
    /// An instance whose source resolves.
    Instance,
    /// An instance whose source is gone or which reads itself (IN-11): it composites nothing.
    Broken,
}

/// Why an instance does not resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Broken {
    /// The linked texture set or layer does not exist.
    Missing,
    /// The instance depends on itself.
    Cycle,
}

/// One instance layer of a document.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceEntry {
    /// Index of the texture set it sits in.
    pub set_index: usize,
    pub set: String,
    pub layer: String,
    pub link: InstanceLink,
    pub broken: Option<Broken>,
}

/// Every instance of a document, for the layer panel's icons and the instance list. Built in one pass over the stacks.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstanceIndex {
    pub entries: Vec<InstanceEntry>,
}

impl InstanceIndex {
    pub fn of(doc: &Document) -> InstanceIndex {
        let mut entries = Vec::new();
        for (set_index, set) in doc.texture_sets.iter().enumerate() {
            for (_, l) in set.walk_layers() {
                if let LayerKind::Instance(link) = &l.kind {
                    let broken = if layer(doc, (&link.set, &link.layer)).is_none() {
                        Some(Broken::Missing)
                    } else if doc.instance_on_cycle(&set.id, &l.id) {
                        Some(Broken::Cycle)
                    } else {
                        None
                    };
                    entries.push(InstanceEntry { set_index, set: set.id.clone(), layer: l.id.clone(), link: link.clone(), broken });
                }
            }
        }
        InstanceIndex { entries }
    }

    /// The instances linking to layer `layer` of set `set`, in document order.
    pub fn instances_of<'a>(&'a self, set: &'a str, layer: &'a str) -> impl Iterator<Item = &'a InstanceEntry> + 'a {
        self.entries.iter().filter(move |e| e.link.set == set && e.link.layer == layer)
    }

    /// The entry of instance layer `layer` in set `set`.
    pub fn entry(&self, set: &str, layer: &str) -> Option<&InstanceEntry> {
        self.entries.iter().find(|e| e.set == set && e.layer == layer)
    }

    pub fn role(&self, set: &str, layer: &str) -> InstanceRole {
        match self.entry(set, layer) {
            Some(e) if e.broken.is_some() => InstanceRole::Broken,
            Some(_) => InstanceRole::Instance,
            None if self.instances_of(set, layer).next().is_some() => InstanceRole::Source,
            None => InstanceRole::Normal,
        }
    }
}

pub(crate) fn layer<'a>(doc: &'a Document, n: Node) -> Option<&'a Layer> {
    doc.texture_set(n.0)?.layer(n.1)
}

/// The layers resolving `n` reads directly: an instance's source, a folder's children.
fn successors<'a>(doc: &'a Document, n: Node) -> Vec<Node<'a>> {
    let Some(set) = doc.texture_set(n.0) else { return Vec::new() };
    let Some(l) = set.layer(n.1) else { return Vec::new() };
    match &l.kind {
        LayerKind::Instance(link) => vec![(link.set.as_str(), link.layer.as_str())],
        LayerKind::Group(children) => children.iter().map(|c| (set.id.as_str(), c.id.as_str())).collect(),
        LayerKind::Paint(_) | LayerKind::Fill(_) => Vec::new(),
    }
}

/// Every layer reachable from `start`'s successors (not `start` itself unless it is on a cycle).
fn reach<'a>(doc: &'a Document, start: Node) -> HashSet<Node<'a>> {
    let mut seen: HashSet<Node<'a>> = HashSet::new();
    let mut todo = successors(doc, start);
    while let Some(n) = todo.pop() {
        if seen.insert(n) {
            todo.extend(successors(doc, n));
        }
    }
    seen
}

impl Document {
    /// True if layer `layer` of set `set` depends on itself through instances (IN-11).
    pub fn instance_on_cycle(&self, set: &str, layer: &str) -> bool {
        reach(self, (set, layer)).contains(&(set, layer))
    }

    /// The indices of the texture sets with an instance whose result reads one of the `changed` layers (texture set
    /// id, layer id), directly or through further instances: the sets beyond the edited one that must be composited
    /// again. A deleted layer is still found while its instances name it. To catch layers added to or removed from a
    /// source folder, name the folder too.
    pub fn instance_dependents(&self, changed: &[(&str, &str)]) -> Vec<usize> {
        let mut out = Vec::new();
        if changed.is_empty() {
            return out;
        }
        for (i, set) in self.texture_sets.iter().enumerate() {
            let hit = set.walk_layers().any(|(_, l)| {
                let LayerKind::Instance(link) = &l.kind else { return false };
                let target = (link.set.as_str(), link.layer.as_str());
                if changed.contains(&target) {
                    return true;
                }
                let read = reach(self, target);
                changed.iter().any(|c| read.contains(c))
            });
            if hit {
                out.push(i);
            }
        }
        out
    }

    /// The layer an instance at the end of a chain of instances links to: `link` followed while it names another
    /// instance. `None` if the chain breaks or loops.
    pub fn instance_source(&self, link: &InstanceLink) -> Option<InstanceLink> {
        let mut at = link.clone();
        let mut seen: Vec<InstanceLink> = Vec::new();
        loop {
            let l = layer(self, (&at.set, &at.layer))?;
            match &l.kind {
                LayerKind::Instance(next) => {
                    if seen.contains(next) {
                        return None;
                    }
                    seen.push(at);
                    at = next.clone();
                }
                _ => return Some(at),
            }
        }
    }

    /// Every layer resolving `link` reads, `link`'s own layer first: the source and, for a folder, its subtree with
    /// what instances in it read. Layers that do not exist are left out.
    pub fn instance_reads(&self, link: &InstanceLink) -> Vec<&Layer> {
        let start = (link.set.as_str(), link.layer.as_str());
        let mut nodes: Vec<Node> = vec![start];
        nodes.extend(reach(self, start).into_iter().filter(|n| *n != start));
        nodes.into_iter().filter_map(|n| layer(self, n)).collect()
    }
}
