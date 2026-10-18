//! Layer instances (IN-01 … IN-13): a layer of kind [`crate::paint::LayerKind::Instance`] names a source layer by
//! [`InstanceLink`] (texture set id, layer id) and takes every parameter from it: blend entries, opacity, mask, effects,
//! fill and, for a folder, the whole subtree, so a layer added to a source folder appears in all its instances. The
//! instance keeps only its own id, name and visibility.
//!
//! - Links are resolved at composite time ([`resolve`]): [`crate::paint::Document::resolved_set`] gives the texture
//!   set with every instance replaced by a copy of its source. Painted pixels are never shared (IN-03): the copy has
//!   no paint images, and its painted mask effects start blank at the instance's resolution. Everything mesh-dependent
//!   is evaluated on the instance's own set (IN-04), because the copy composites inside it.
//! - [`graph`]: which layers a layer's resolution reads. An instance that reads itself (an instance inside the folder
//!   it depends on, directly or through other instances) is on a cycle: it is broken and composites nothing (IN-11);
//!   edits refuse to make one. [`crate::paint::Document::instance_dependents`] lists the texture sets an edit of some
//!   layers reaches through instances, so only those are composited again (IN-13).
//!
//! The document's instance problems (broken links, cycles, channels a target set lacks) are listed by
//! [`crate::paint::validate::instance_issues`]; they are reported, not refused, so a document with a broken instance
//! still opens and saves.

mod graph;
mod resolve;

#[cfg(test)]
mod tests;

pub use graph::{Broken, InstanceEntry, InstanceIndex, InstanceRole};
pub use resolve::CHILD_ID_SEPARATOR;

/// Which layer an instance takes its parameters from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceLink {
    /// Texture set id.
    pub set: String,
    /// Layer id within that set.
    pub layer: String,
}

impl InstanceLink {
    pub fn new(set: impl Into<String>, layer: impl Into<String>) -> InstanceLink {
        InstanceLink { set: set.into(), layer: layer.into() }
    }
}
