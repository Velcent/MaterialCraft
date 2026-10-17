//! [`Filter`]: what the user narrowed the tray to: usage types (multi-select, AS-12), a folder (AS-11, AS-13) and
//! the query text (AS-14, AS-15), combined with AND into one [`Search`]; and its conversion to and from a saved
//! search as the settings file stores it (AS-16).

use materialcraft::assets::{Location, Query, RootId, Search, Usage};

use crate::gui_settings::SavedSearch;

/// The tray's current filter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Usages to show; empty shows all.
    pub usages: Vec<Usage>,
    pub location: Option<Location>,
    /// The query exactly as typed.
    pub query: String,
}

impl Filter {
    /// The search this filter runs.
    pub fn search(&self) -> Search {
        Search { usages: self.usages.clone(), location: self.location.clone(), query: Query::parse(&self.query), hide_importable: false }
    }

    /// Turns `usage` on or off in the type filter.
    pub fn toggle(&mut self, usage: Usage) {
        match self.usages.iter().position(|u| *u == usage) {
            Some(i) => {
                self.usages.remove(i);
            }
            None => {
                self.usages.push(usage);
                self.usages.sort();
            }
        }
    }

    pub fn shows(&self, usage: Usage) -> bool {
        self.usages.contains(&usage)
    }

    /// The filter a saved search stands for; unknown usage keywords are left out.
    pub fn from_saved(s: &SavedSearch) -> Filter {
        let (mut usages, _) = materialcraft::assets::SavedSearch::usages_from_names(s.usages.iter().map(String::as_str));
        usages.sort();
        let location = (!s.library.is_empty()).then(|| Location::folder(RootId::new(s.library.clone()), s.folder.clone()));
        Filter { usages, location, query: s.query.clone() }
    }

    /// This filter as a saved search called `name`.
    pub fn to_saved(&self, name: &str, tab: bool) -> SavedSearch {
        let (library, folder) = match &self.location {
            Some(l) => (l.root.as_str().to_owned(), l.folder.clone()),
            None => (String::new(), String::new()),
        };
        SavedSearch {
            name: name.to_owned(),
            query: self.query.clone(),
            usages: self.usages.iter().map(|u| u.keyword().to_owned()).collect(),
            library,
            folder,
            tab,
        }
    }

    /// A one-line description of the filter (shown when hovering a saved search).
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if !self.usages.is_empty() {
            parts.push(format!("types: {}", self.usages.iter().map(|u| u.keyword()).collect::<Vec<_>>().join(", ")));
        }
        if let Some(l) = &self.location {
            parts.push(format!("in: {}/{}", l.root, l.folder).trim_end_matches('/').to_owned());
        }
        parts.push(if self.query.is_empty() { "query: (none)".to_owned() } else { format!("query: {}", self.query) });
        parts.join("   ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_survives_the_settings_file() {
        let mut f = Filter { query: "!rust -name:old".into(), ..Filter::default() };
        f.toggle(Usage::SmartMask);
        f.toggle(Usage::Material);
        f.location = Some(Location::folder(RootId::new("user"), "materials/metal"));
        let saved = f.to_saved("Metals", true);
        assert_eq!(saved.usages, ["material", "smart-mask"]);
        assert_eq!(Filter::from_saved(&saved), f);
        f.toggle(Usage::Material);
        assert_eq!(f.usages, [Usage::SmartMask]);
        assert!(Filter::default().describe().contains("(none)"));
    }
}
