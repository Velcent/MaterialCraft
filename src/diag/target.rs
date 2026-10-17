//! What an entry is about, so selecting it in the panel can go there (LG-10, BK-09).

use std::fmt;

/// The object an entry concerns. Ids are the document's own string ids; this module knows
/// nothing about the paint model, so it neither checks nor resolves them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Target {
    /// A texture set, by id.
    TextureSet(String),
    /// A layer, by texture set id and layer id.
    Layer { texture_set: String, layer: String },
    /// A bake setting, by its settings key.
    BakeSetting(String),
    /// A library asset, by path or id.
    Asset(String),
    /// A file on disk (an imported project, a mesh, an export destination).
    File(String),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::TextureSet(id) => write!(f, "texture set {id}"),
            Target::Layer { texture_set, layer } => write!(f, "layer {layer} in {texture_set}"),
            Target::BakeSetting(key) => write!(f, "bake setting {key}"),
            Target::Asset(id) => write!(f, "asset {id}"),
            Target::File(path) => write!(f, "file {path}"),
        }
    }
}
