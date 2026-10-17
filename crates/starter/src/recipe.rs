//! [`Recipe`]: one starter asset, made from fixed inputs. [`Made`] is what it makes: the asset's bytes, its sidecar
//! and its thumbnail.

use crate::StarterError;
use crate::build::Options;
use materialcraft::assets::{Provenance, Sidecar, Usage};

/// One starter asset.
pub trait Recipe: Send + Sync {
    /// The asset's root-relative path (`/`-separated), in the folder of its usage.
    fn path(&self) -> String;

    /// The asset, its sidecar and its thumbnail. Same options, same bytes.
    fn make(&self, options: &Options) -> Result<Made, StarterError>;
}

/// A made asset.
#[derive(Debug, Clone, PartialEq)]
pub struct Made {
    pub bytes: Vec<u8>,
    pub sidecar: Sidecar,
    /// The conventional thumbnail (`<file>.thumb.png`), or `None` when the asset holds its own (fragments).
    pub thumbnail: Option<Vec<u8>>,
}

/// The sidecar of a starter asset: its name, usage, tags, a description and how it was made.
pub(crate) fn sidecar(name: &str, usage: Usage, tags: &[&str], description: &str, provenance: Provenance) -> Sidecar {
    Sidecar {
        name: Some(name.to_owned()),
        usage: Some(usage),
        tags: tags.iter().map(|t| (*t).to_owned()).collect(),
        description: Some(description.to_owned()),
        provenance,
        ..Sidecar::default()
    }
}
