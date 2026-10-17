//! Matching one term against one asset. Values and keys are both lowercase already.

use super::{Field, Mode, Term};
use crate::assets::{Asset, LibraryRoot, Usage};

/// Lowercase keys of the root an asset belongs to, computed once per query.
pub(crate) struct RootKeys {
    pub name: String,
    pub kind: &'static str,
}

impl RootKeys {
    pub fn of(root: &LibraryRoot) -> RootKeys {
        RootKeys { name: root.name.to_lowercase(), kind: root.kind.name() }
    }
}

pub(super) fn term_matches(t: &Term, a: &Asset, root: &RootKeys) -> bool {
    let k = &a.keys;
    let v = t.value.as_str();
    let cmp = |s: &str| match t.mode {
        Mode::Contains => s.contains(v),
        Mode::Exact => s == v,
    };
    let path = || match t.mode {
        // Exact on a path matches the whole folder or any one folder name.
        Mode::Contains => k.folder.contains(v),
        Mode::Exact => k.folder == v || k.folder.split('/').any(|s| s == v),
    };
    match t.field {
        Field::Name => cmp(&k.name),
        Field::Tag => k.tags.iter().any(|s| cmp(s)),
        Field::Path => path(),
        Field::Library => cmp(&root.name) || cmp(root.kind),
        Field::Usage => match t.mode {
            Mode::Contains => a.usage.keyword().contains(v),
            Mode::Exact => Usage::from_name(v) == Some(a.usage),
        },
        Field::Format => cmp(a.format.extension()),
        Field::Source => cmp(a.provenance.kind()),
        Field::Any => match t.mode {
            Mode::Contains => k.name.contains(v) || k.file.contains(v) || k.tags.iter().any(|s| s.contains(v)) || path() || root.name.contains(v),
            Mode::Exact => k.name == v || k.file == v || k.tags.iter().any(|s| s == v) || path(),
        },
    }
}
