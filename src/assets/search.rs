//! What the tray asks the catalogue: type filter, folder filter and query text combined with AND
//! (AS-12), and saved searches as plain data (AS-16). The editor stores saved searches in its
//! settings file; this module only defines and compiles them.

use super::{Query, RootId, Usage};

/// A folder filter: one library, optionally narrowed to a folder and everything under it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Location {
    pub root: RootId,
    /// Root-relative folder (`/`-separated); empty for the whole library.
    pub folder: String,
}

impl Location {
    pub fn root(root: RootId) -> Location {
        Location { root, folder: String::new() }
    }

    pub fn folder(root: RootId, folder: impl Into<String>) -> Location {
        Location { root, folder: folder.into().trim_matches('/').to_owned() }
    }

    /// True if an asset in `root` at folder `folder` lies inside this location.
    pub fn contains(&self, root: &RootId, folder: &str) -> bool {
        if &self.root != root {
            return false;
        }
        self.folder.is_empty() || folder.strip_prefix(self.folder.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }
}

/// A compiled search.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Search {
    /// Usages to show; empty shows all (AS-12 multi-select).
    pub usages: Vec<Usage>,
    /// Folder filter; `None` searches every library.
    pub location: Option<Location>,
    pub query: Query,
    /// Leave out files MaterialCraft can only offer to import (`.sbsar`). Off by default: they are
    /// shown, marked importable.
    pub hide_importable: bool,
}

impl Search {
    /// Everything.
    pub fn all() -> Search {
        Search::default()
    }

    /// A search for `text` over every library and usage.
    pub fn text(text: &str) -> Search {
        Search { query: Query::parse(text), ..Search::default() }
    }

    pub fn with_usages(mut self, usages: impl IntoIterator<Item = Usage>) -> Search {
        self.usages = usages.into_iter().collect();
        self
    }

    pub fn with_location(mut self, location: Location) -> Search {
        self.location = Some(location);
        self
    }

    pub fn allows_usage(&self, usage: Usage) -> bool {
        self.usages.is_empty() || self.usages.contains(&usage)
    }
}

/// A named search the user kept (AS-16): type filter, folder and query text. Hovering it in the
/// tray shows `query`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavedSearch {
    pub name: String,
    /// Usages to show; empty shows all. [`SavedSearch::usages_from_names`] reads them from settings.
    pub usages: Vec<Usage>,
    pub location: Option<Location>,
    /// The query text exactly as typed.
    pub query: String,
}

impl SavedSearch {
    /// The search this saved search runs.
    pub fn compile(&self) -> Search {
        Search { usages: self.usages.clone(), location: self.location.clone(), query: Query::parse(&self.query), hide_importable: false }
    }

    /// Read usage keywords from settings, returning the ones that aren't known so the editor can
    /// report them.
    pub fn usages_from_names<'a>(names: impl IntoIterator<Item = &'a str>) -> (Vec<Usage>, Vec<&'a str>) {
        let mut known = Vec::new();
        let mut unknown = Vec::new();
        for n in names {
            match Usage::from_name(n) {
                Some(u) if !known.contains(&u) => known.push(u),
                Some(_) => {}
                None => unknown.push(n),
            }
        }
        (known, unknown)
    }
}
