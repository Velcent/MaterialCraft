//! [`Results`]: what the tray shows for the current search, copied out of the catalogue once per change (a new
//! search or a library change), so drawing never holds the library's lock. Besides the matching assets it keeps
//! each library's folders and how many assets in each folder match the type filter and query, for the
//! breadcrumbs, the greyed-out folders (AS-11) and "hide empty" (AS-13).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;

use materialcraft::assets::{Asset, Format, Hit, Library, Provenance, RootId, Search, Thumbnail, Usage};
use materialcraft::json::Value;

/// Where an asset's picture comes from.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ThumbSource {
    /// Nothing to show but the usage icon.
    None,
    /// An image file (the asset itself or its thumbnail).
    Image(PathBuf),
    /// A material graph, rendered small.
    Material(PathBuf),
    /// A smart material or smart mask file holding its own thumbnail.
    Fragment(PathBuf),
}

/// One asset as the tray shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub root: RootId,
    /// Root-relative path, `/`-separated.
    pub path: String,
    pub name: String,
    pub usage: Usage,
    pub format: Format,
    /// The library's display name.
    pub library: String,
    /// The library's folder, if it has one.
    pub root_dir: Option<PathBuf>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub thumb: ThumbSource,
    /// Modification time and size: a changed file gets a new thumbnail.
    pub stamp: (Option<SystemTime>, u64),
    /// The built-in generator or filter and its preset values, if the asset was made by one (its provenance).
    pub procedural: Option<(String, Vec<(String, String)>)>,
}

impl Item {
    fn of(hit: &Hit) -> Item {
        let (asset, root) = (hit.asset, hit.root);
        Item {
            root: root.id.clone(),
            path: asset.path.clone(),
            name: asset.name.clone(),
            usage: asset.usage,
            format: asset.format,
            library: root.name.clone(),
            root_dir: root.path.clone(),
            tags: asset.tags.clone(),
            description: asset.description.clone(),
            thumb: thumb_source(root.path.as_deref(), asset),
            stamp: (asset.modified, asset.size),
            procedural: procedural(&asset.provenance),
        }
    }

    /// True if MaterialCraft can only offer to import it (`.sbsar`, ADR 0008).
    pub fn importable_only(&self) -> bool {
        !self.format.is_native()
    }
}

fn procedural(p: &Provenance) -> Option<(String, Vec<(String, String)>)> {
    let Provenance::Generator { generator, preset } = p else { return None };
    let text = |v: &Value| match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    };
    let params = preset.as_ref().and_then(Value::as_object).map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), text(v)?))).collect()).unwrap_or_default();
    Some((generator.clone(), params))
}

fn thumb_source(dir: Option<&std::path::Path>, asset: &Asset) -> ThumbSource {
    let Some(dir) = dir else { return ThumbSource::None };
    match &asset.thumbnail {
        Thumbnail::File(rel) => ThumbSource::Image(dir.join(rel)),
        Thumbnail::OwnImage => ThumbSource::Image(dir.join(&asset.path)),
        Thumbnail::Embedded => ThumbSource::Fragment(dir.join(&asset.path)),
        Thumbnail::None if asset.format == Format::Graph => ThumbSource::Material(dir.join(&asset.path)),
        Thumbnail::None => ThumbSource::None,
    }
}

/// A library and its folders, in path order.
#[derive(Clone, Debug, PartialEq)]
pub struct RootFolders {
    pub id: RootId,
    pub name: String,
    pub folders: Vec<String>,
}

/// The current search's results.
#[derive(Default)]
pub struct Results {
    key: Option<(u64, Search)>,
    pub items: Vec<Item>,
    pub roots: Vec<RootFolders>,
    /// Matching assets per (library, folder), counting everything below the folder; the folder `""` is the
    /// whole library.
    counts: HashMap<(RootId, String), usize>,
}

impl Results {
    /// True if the results are those of `search` at library `generation`.
    pub fn is_current(&self, generation: u64, search: &Search) -> bool {
        self.key.as_ref().is_some_and(|(g, s)| *g == generation && s == search)
    }

    /// Recomputes the results from `library`.
    pub fn refresh(&mut self, library: &Library, generation: u64, search: &Search) {
        let catalogue = library.catalogue();
        let broad = Search { location: None, ..search.clone() };
        self.items.clear();
        self.counts.clear();
        for hit in catalogue.find(&broad) {
            let folder = hit.asset.folder();
            let mut f = folder;
            loop {
                *self.counts.entry((hit.root.id.clone(), f.to_owned())).or_default() += 1;
                if f.is_empty() {
                    break;
                }
                f = f.rsplit_once('/').map_or("", |(parent, _)| parent);
            }
            if search.location.as_ref().is_none_or(|l| l.contains(&hit.root.id, folder)) {
                self.items.push(Item::of(&hit));
            }
        }
        self.roots = catalogue
            .roots()
            .map(|r| RootFolders { id: r.id.clone(), name: r.name.clone(), folders: catalogue.folders(&r.id).map(str::to_owned).collect() })
            .collect();
        self.key = Some((generation, search.clone()));
    }

    /// How many assets in `folder` of library `root` (and below) match the type filter and query.
    pub fn count(&self, root: &RootId, folder: &str) -> usize {
        self.counts.get(&(root.clone(), folder.to_owned())).copied().unwrap_or(0)
    }
}
