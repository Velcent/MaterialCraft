//! Finding a material's graph from the reference a fill stores ([`crate::paint::FillSource::Material`]'s
//! `path`): [`MaterialResolver`] and two implementations, [`FolderResolver`] (files under a list of folders: the
//! document's folder, asset library roots) and [`MemoryResolver`] (graphs held in memory: tests, the session, a
//! document's embedded assets), chained by [`ChainResolver`].

use crate::paint::EmbeddedAsset;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// Largest `.mca` file read (embedded resources make graphs large; anything bigger is refused).
pub const MAX_GRAPH_BYTES: u64 = 256 << 20;

/// Why a reference does not lead to a graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// No folder holds the file, or no graph has that name.
    NotFound { reference: String },
    /// The file exists but cannot be read (I/O error, too large, not UTF-8).
    Unreadable { reference: String, reason: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::NotFound { reference } => write!(f, "material {reference:?} not found"),
            ResolveError::Unreadable { reference, reason } => write!(f, "material {reference:?} cannot be read: {reason}"),
        }
    }
}

/// Turns a material reference into the text of its `.mca` graph.
pub trait MaterialResolver: Send + Sync {
    fn resolve(&self, reference: &str) -> Result<String, ResolveError>;
}

/// Looks references up as files: an absolute path as it is, a relative one (`/`-separated, no `..`) under each
/// folder in turn; the first folder that has the file wins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FolderResolver {
    folders: Vec<PathBuf>,
}

impl FolderResolver {
    pub fn new(folders: impl IntoIterator<Item = PathBuf>) -> FolderResolver {
        FolderResolver { folders: folders.into_iter().collect() }
    }

    pub fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    /// The file `reference` names, if one exists.
    pub fn locate(&self, reference: &str) -> Option<PathBuf> {
        let path = Path::new(reference);
        if path.is_absolute() {
            return path.is_file().then(|| path.to_path_buf());
        }
        let relative = path.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if !relative || reference.is_empty() {
            return None;
        }
        self.folders.iter().map(|f| f.join(path)).find(|p| p.is_file())
    }
}

impl MaterialResolver for FolderResolver {
    fn resolve(&self, reference: &str) -> Result<String, ResolveError> {
        let path = self.locate(reference).ok_or_else(|| ResolveError::NotFound { reference: reference.to_string() })?;
        let unreadable = |reason: String| ResolveError::Unreadable { reference: reference.to_string(), reason };
        let len = std::fs::metadata(&path).map_err(|e| unreadable(e.to_string()))?.len();
        if len > MAX_GRAPH_BYTES {
            return Err(unreadable(format!("{len} bytes is more than the {MAX_GRAPH_BYTES} read")));
        }
        std::fs::read_to_string(&path).map_err(|e| unreadable(e.to_string()))
    }
}

/// Graph texts by reference.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MemoryResolver {
    graphs: BTreeMap<String, String>,
}

impl MemoryResolver {
    pub fn new() -> MemoryResolver {
        MemoryResolver::default()
    }

    /// Builder: `reference` resolves to `graph` (the `.mca` text).
    pub fn with(mut self, reference: impl Into<String>, graph: impl Into<String>) -> MemoryResolver {
        self.insert(reference, graph);
        self
    }

    pub fn insert(&mut self, reference: impl Into<String>, graph: impl Into<String>) {
        self.graphs.insert(reference.into(), graph.into());
    }

    /// The material graphs among a document's embedded assets (`.mca` references holding UTF-8 text), so a
    /// document opens without the library it was made with (AS-21).
    pub fn from_embedded(assets: &[EmbeddedAsset]) -> MemoryResolver {
        let mut r = MemoryResolver::new();
        for a in assets.iter().filter(|a| a.reference.to_ascii_lowercase().ends_with(".mca")) {
            if let Ok(text) = std::str::from_utf8(&a.bytes) {
                r.insert(a.reference.clone(), text);
            }
        }
        r
    }

    pub fn is_empty(&self) -> bool {
        self.graphs.is_empty()
    }
}

impl MaterialResolver for MemoryResolver {
    fn resolve(&self, reference: &str) -> Result<String, ResolveError> {
        self.graphs.get(reference).cloned().ok_or_else(|| ResolveError::NotFound { reference: reference.to_string() })
    }
}

/// Several resolvers asked in turn: the first that finds the reference wins. When none does, the first error
/// other than "not found" is reported (a file that exists but cannot be read says more than a missing one).
#[derive(Clone, Default)]
pub struct ChainResolver {
    resolvers: Vec<Arc<dyn MaterialResolver>>,
}

impl ChainResolver {
    pub fn new(resolvers: impl IntoIterator<Item = Arc<dyn MaterialResolver>>) -> ChainResolver {
        ChainResolver { resolvers: resolvers.into_iter().collect() }
    }
}

impl MaterialResolver for ChainResolver {
    fn resolve(&self, reference: &str) -> Result<String, ResolveError> {
        let mut unreadable = None;
        for r in &self.resolvers {
            match r.resolve(reference) {
                Ok(text) => return Ok(text),
                Err(ResolveError::NotFound { .. }) => {}
                Err(e) => {
                    unreadable.get_or_insert(e);
                }
            }
        }
        Err(unreadable.unwrap_or_else(|| ResolveError::NotFound { reference: reference.to_string() }))
    }
}
