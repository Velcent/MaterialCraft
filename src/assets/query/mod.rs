//! The tray's search query (AS-14, AS-15): parsed once, then matched against every asset's
//! precomputed lowercase keys. See [`parse`](self::parse) for the syntax.

mod matcher;
mod parse;

pub(crate) use matcher::RootKeys;

/// Which asset field a term looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field {
    /// Name, file name, tags, folder and library (AS-14: name, location and tags).
    Any,
    Name,
    Tag,
    /// The asset's folder within its library.
    Path,
    /// The library's name or kind (`starter`, `user`, …).
    Library,
    Usage,
    /// The file format's extension.
    Format,
    /// The provenance kind (`graph`, `generator`, `hand-made`, `cc0`, `unknown`).
    Source,
}

impl Field {
    /// The field named by a query prefix (without the colon), ignoring case.
    pub fn from_prefix(prefix: &str) -> Option<Field> {
        Some(match prefix.to_ascii_lowercase().as_str() {
            "name" => Field::Name,
            "tag" | "tags" => Field::Tag,
            "path" | "folder" => Field::Path,
            "library" | "lib" => Field::Library,
            "usage" | "type" => Field::Usage,
            "format" | "ext" => Field::Format,
            "source" | "provenance" => Field::Source,
            _ => return None,
        })
    }
}

/// How a term's value is compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The field contains the value (plain words, `*word*`).
    Contains,
    /// The field equals the value (`!word`).
    Exact,
    /// The field starts with the value (`word*`).
    StartsWith,
    /// The field ends with the value (`*word`).
    EndsWith,
}

impl Mode {
    /// True if `field` holds `value` in this mode (both lowercase already).
    pub fn holds(self, field: &str, value: &str) -> bool {
        match self {
            Mode::Contains => field.contains(value),
            Mode::Exact => field == value,
            Mode::StartsWith => field.starts_with(value),
            Mode::EndsWith => field.ends_with(value),
        }
    }
}

/// One search term. Values are stored lowercase; matching ignores case.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Term {
    pub field: Field,
    pub mode: Mode,
    pub value: String,
    /// True for `-term`: the asset must *not* match.
    pub exclude: bool,
}

/// A parsed search: all terms must hold. The empty query matches everything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Query {
    terms: Vec<Term>,
}

impl Query {
    /// Parse search text. Never fails: see the module docs for how odd input is read.
    pub fn parse(text: &str) -> Query {
        Query { terms: parse::parse(text) }
    }

    /// A query from terms built in code (values are lowercased).
    pub fn from_terms(terms: impl IntoIterator<Item = Term>) -> Query {
        Query { terms: terms.into_iter().map(|t| Term { value: t.value.to_lowercase(), ..t }).collect() }
    }

    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// True if `asset` in a root described by `root` satisfies every term.
    pub(crate) fn matches(&self, asset: &super::Asset, root: &RootKeys) -> bool {
        self.terms.iter().all(|t| matcher::term_matches(t, asset, root) != t.exclude)
    }
}
