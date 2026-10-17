//! Library roots (AS-01, AS-18, AS-21). The editor reads them from its settings and passes them in
//! as plain values (ADR 0008, rm-settings-store); this module never stores settings itself.

use std::fmt;
use std::path::PathBuf;

/// Stable id of a library root, chosen by the editor (for example the settings key).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RootId(pub String);

impl RootId {
    pub fn new(id: impl Into<String>) -> RootId {
        RootId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RootId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which storage tier or scope a root is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RootKind {
    /// Shipped with MaterialCraft in the install folder; read-only.
    Starter,
    /// The user's own library (Documents by default).
    User,
    /// A further library added in the settings.
    Extra,
    /// Assets of the current session.
    Session,
    /// Assets embedded in the open project.
    Project,
}

impl RootKind {
    /// Stable lowercase name, also matched by the `library:` query field.
    pub fn name(self) -> &'static str {
        match self {
            RootKind::Starter => "starter",
            RootKind::User => "user",
            RootKind::Extra => "extra",
            RootKind::Session => "session",
            RootKind::Project => "project",
        }
    }
}

/// One library: a folder on disk, or (for session and project scopes) an in-memory list of assets
/// the editor hands to the catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRoot {
    pub id: RootId,
    /// Display name, matched by the `library:` query field.
    pub name: String,
    pub kind: RootKind,
    /// The folder, or `None` for a root whose assets are supplied in memory.
    pub path: Option<PathBuf>,
}

impl LibraryRoot {
    /// A folder-backed root.
    pub fn folder(id: impl Into<String>, name: impl Into<String>, kind: RootKind, path: impl Into<PathBuf>) -> LibraryRoot {
        LibraryRoot { id: RootId::new(id), name: name.into(), kind, path: Some(path.into()) }
    }

    /// A root without a folder; its assets are set with `Catalogue::set_assets`.
    pub fn in_memory(id: impl Into<String>, name: impl Into<String>, kind: RootKind) -> LibraryRoot {
        LibraryRoot { id: RootId::new(id), name: name.into(), kind, path: None }
    }

    /// The starter library is never written to.
    pub fn is_read_only(&self) -> bool {
        self.kind == RootKind::Starter
    }
}
