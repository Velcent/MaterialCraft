//! [`CompositeState`]: what the composited maps of a document depend on, kept before a change whose reach is not
//! known (an undo, a redo, a jump in the history), so that afterwards only the texture sets that changed are
//! composited again instead of all of them.
//!
//! Keeping it is cheap: texture sets share their image tiles with the document (copy-on-write), and comparing
//! shared tiles compares pointers.

use super::doc::Document;
use super::height_normal::HeightFilter;
use super::texture_set::TextureSet;

/// The parts of a document compositing reads.
#[derive(Debug, Clone)]
pub struct CompositeState {
    sets: Vec<TextureSet>,
    height_filter: HeightFilter,
    /// Embedded assets by reference and size (material fills read them).
    assets: Vec<(String, usize)>,
}

impl CompositeState {
    pub fn of(doc: &Document) -> CompositeState {
        CompositeState { sets: doc.texture_sets.clone(), height_filter: doc.height_filter, assets: assets_of(doc) }
    }

    /// The indices (in `doc`) of the texture sets whose maps may differ from the kept state's, ascending: sets that
    /// changed or are new, and sets with instances reading a changed set's layers. Every set when a document-wide
    /// input changed (the height filter, the embedded assets).
    pub fn changed_sets(&self, doc: &Document) -> Vec<usize> {
        if doc.height_filter != self.height_filter || assets_of(doc) != self.assets {
            return (0..doc.texture_sets.len()).collect();
        }
        let mut changed: Vec<usize> = Vec::new();
        let mut layers: Vec<(String, String)> = Vec::new();
        for (i, set) in doc.texture_sets.iter().enumerate() {
            match self.sets.iter().find(|s| s.id == set.id) {
                Some(before) if before == set => continue,
                Some(before) => layers.extend(before.walk_layers().map(|(_, l)| (before.id.clone(), l.id.clone()))),
                None => {}
            }
            layers.extend(set.walk_layers().map(|(_, l)| (set.id.clone(), l.id.clone())));
            changed.push(i);
        }
        let named: Vec<(&str, &str)> = layers.iter().map(|(s, l)| (s.as_str(), l.as_str())).collect();
        changed.extend(doc.instance_dependents(&named));
        changed.sort_unstable();
        changed.dedup();
        changed
    }
}

fn assets_of(doc: &Document) -> Vec<(String, usize)> {
    doc.assets.iter().map(|a| (a.reference.clone(), a.bytes.len())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::{BlendMode, Channel, Fill, Layer, LayerKind};

    fn doc() -> Document {
        let mut d = Document::default();
        for id in ["a", "b", "c"] {
            let mut s = TextureSet::new(id, id, (8, 8));
            let fill = Fill::uniform(vec![(Channel::BaseColor, [0.5; 4])]);
            s.layers.push(Layer::new(format!("{id}-fill"), "Fill", LayerKind::Fill(fill)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
            d.texture_sets.push(s);
        }
        d
    }

    #[test]
    fn only_changed_sets_are_named() {
        let mut d = doc();
        let before = CompositeState::of(&d);
        assert!(before.changed_sets(&d).is_empty());
        d.texture_sets[1].layers[0].opacity = 0.5;
        assert_eq!(before.changed_sets(&d), vec![1]);
        d.texture_sets.push(TextureSet::new("d", "d", (8, 8)));
        assert_eq!(before.changed_sets(&d), vec![1, 3]);
        d.height_filter = match d.height_filter {
            HeightFilter::Sobel => HeightFilter::Backward,
            HeightFilter::Backward => HeightFilter::Sobel,
        };
        assert_eq!(before.changed_sets(&d), vec![0, 1, 2, 3]);
    }
}
