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
    let cmp = |s: &str| t.mode.holds(s, v);
    let path = || match t.mode {
        Mode::Contains => k.folder.contains(v),
        // Exact, starts and ends with on a path look at the whole folder or any one folder name.
        m => m.holds(&k.folder, v) || k.folder.split('/').any(|s| m.holds(s, v)),
    };
    match t.field {
        Field::Name => cmp(&k.name),
        Field::Tag => k.tags.iter().any(|s| cmp(s)),
        Field::Path => path(),
        Field::Library => cmp(&root.name) || cmp(root.kind),
        Field::Usage => match t.mode {
            Mode::Exact => Usage::from_name(v) == Some(a.usage),
            m => m.holds(a.usage.keyword(), v),
        },
        Field::Format => cmp(a.format.extension()),
        Field::Source => cmp(a.provenance.kind()),
        Field::Any => match t.mode {
            // The library's name counts for the loose modes only, as before for plain words.
            Mode::Exact => k.name == v || k.file == v || k.tags.iter().any(|s| s == v) || path(),
            m => m.holds(&k.name, v) || m.holds(&k.file, v) || k.tags.iter().any(|s| m.holds(s, v)) || path() || m.holds(&root.name, v),
        },
    }
}
