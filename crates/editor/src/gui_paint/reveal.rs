//! Going to what a log entry concerns (LG-10): resolving a [`Target`] against the open document,
//! and the short highlight of the texture set it leads to.

use std::time::{Duration, Instant};

use materialcraft::diag::Target;
use materialcraft::paint::bake_config::SettingKey;

use super::document_state::OpenDocument;

/// How long the texture set a log entry led to stays highlighted.
const HIGHLIGHT: Duration = Duration::from_millis(1600);

/// Where a target leads on the Paint screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The texture set at this index: shown in the texture set list.
    TextureSet(usize),
    /// A layer of the texture set at `set`: selected in the layer panel, its texture set shown.
    Layer { set: usize, layer: String },
    /// A bake setting (`<section>.<field>`) of the texture set at `set`: shown in the bakers panel.
    BakeSetting { set: usize, field: String },
    /// Nothing on this screen to go to; the text says why.
    Nowhere(String),
}

/// Where `target` leads with `doc` open.
pub fn resolve(target: &Target, doc: Option<&OpenDocument>) -> Destination {
    let set = |id: &str| doc.and_then(|d| d.set_index(id));
    match target {
        Target::TextureSet(id) => match set(id) {
            Some(i) => Destination::TextureSet(i),
            None => Destination::Nowhere(format!("Texture set {id} is not in the open document.")),
        },
        Target::Layer { texture_set, layer } => match set(texture_set) {
            Some(i) => Destination::Layer { set: i, layer: layer.clone() },
            None => Destination::Nowhere(format!("Texture set {texture_set} is not in the open document.")),
        },
        Target::BakeSetting(key) => match SettingKey::parse(key) {
            Some(k) => match set(&k.set) {
                Some(i) => Destination::BakeSetting { set: i, field: k.field },
                None => Destination::Nowhere(format!("Texture set {} is not in the open document.", k.set)),
            },
            None => Destination::Nowhere(format!("The bake setting {key} belongs to no texture set.")),
        },
        Target::Asset(id) => Destination::Nowhere(format!("Asset {id} has no panel to show it yet.")),
        Target::File(path) => Destination::Nowhere(format!("The message is about the file {path}.")),
    }
}

/// A texture set highlighted after going to it from the log.
#[derive(Debug, Clone, Copy)]
pub struct Focus {
    set: usize,
    since: Instant,
    scrolled: bool,
}

impl Focus {
    pub fn new(set: usize) -> Focus {
        Focus { set, since: Instant::now(), scrolled: false }
    }

    /// The set to highlight this frame and whether to scroll it into view (the first frame
    /// only); `None` once the highlight is over.
    pub fn frame(&mut self) -> Option<(usize, bool)> {
        if self.since.elapsed() > HIGHLIGHT {
            return None;
        }
        let scroll = !std::mem::replace(&mut self.scrolled, true);
        Some((self.set, scroll))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::{Document, TextureSet};

    #[test]
    fn targets_resolve_against_the_open_document() {
        let mut doc = Document::default();
        doc.texture_sets.push(TextureSet::new("a", "Body", (4, 4)));
        doc.texture_sets.push(TextureSet::new("b", "Head", (4, 4)));
        let open = OpenDocument::new(doc, "x.mcpaint".into(), None);
        assert_eq!(resolve(&Target::TextureSet("b".into()), Some(&open)), Destination::TextureSet(1));
        let layer = Target::Layer { texture_set: "a".into(), layer: "l1".into() };
        assert_eq!(resolve(&layer, Some(&open)), Destination::Layer { set: 0, layer: "l1".into() });
        assert!(matches!(resolve(&layer, None), Destination::Nowhere(_)));
        assert!(matches!(resolve(&Target::TextureSet("zz".into()), Some(&open)), Destination::Nowhere(_)));
        let key = Target::BakeSetting("ambient_occlusion.rays@b".into());
        assert_eq!(resolve(&key, Some(&open)), Destination::BakeSetting { set: 1, field: "ambient_occlusion.rays".into() });
        assert!(matches!(resolve(&Target::BakeSetting("max_dist".into()), Some(&open)), Destination::Nowhere(_)));
    }

    #[test]
    fn focus_scrolls_once_then_fades() {
        let mut f = Focus::new(3);
        assert_eq!(f.frame(), Some((3, true)));
        assert_eq!(f.frame(), Some((3, false)));
        f.since -= HIGHLIGHT * 2;
        assert_eq!(f.frame(), None);
    }
}
