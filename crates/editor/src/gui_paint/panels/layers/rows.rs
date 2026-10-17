//! [`Rows`]: the stack as the panel lists it, top of the stack first: a row per layer (children indented
//! under their folder unless it is folded) and, for layers whose effects are unfolded, a row per effect.
//! Rows have fixed heights, so the list knows every row's position without drawing it (only the rows in
//! view are drawn) and can turn a pointer position into a drop target (LS-14, LS-15, LS-19).

use materialcraft::paint::edit::ops::DropTarget;
use materialcraft::paint::edit::{LayerRef, LayerSlot, StackId};
use materialcraft::paint::{Layer, LayerKind, TextureSet};
use std::collections::HashSet;

pub const LAYER_ROW: f32 = 44.0;
pub const EFFECT_ROW: f32 = 22.0;
/// Width of one nesting level.
pub const INDENT: f32 = 14.0;
/// Fraction of a layer row's height at its top and bottom edges that means "between".
const EDGE: f32 = 0.25;

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    Layer {
        id: String,
        /// Where the layer sits.
        slot: LayerSlot,
        /// A folder with children shown below it.
        open_folder: bool,
    },
    Effect {
        layer: String,
        stack: StackId,
        index: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub kind: RowKind,
    pub depth: usize,
    pub top: f32,
    pub height: f32,
}

/// Which folders are folded and which layers show their effects (panel state, by layer id).
#[derive(Clone, Debug, Default)]
pub struct Folding {
    pub folded: HashSet<String>,
    pub effects_open: HashSet<String>,
}

/// The listed rows of one texture set.
#[derive(Clone, Debug, Default)]
pub struct Rows {
    pub set: String,
    pub rows: Vec<Row>,
    pub height: f32,
}

/// Where a dragged thing would land.
#[derive(Clone, Debug, PartialEq)]
pub struct Landing {
    pub target: DropTarget,
    /// The y of the marker line for a between-rows drop; `None` when the drop is onto a row.
    pub line: Option<f32>,
    /// The row a drop onto a layer or mask highlights.
    pub row: Option<usize>,
}

impl Rows {
    pub fn build(set: &TextureSet, folding: &Folding) -> Rows {
        let mut rows = Rows { set: set.id.clone(), rows: Vec::new(), height: 0.0 };
        rows.add_layers(&set.layers, None, 0, folding);
        rows
    }

    fn push(&mut self, kind: RowKind, depth: usize, height: f32) {
        self.rows.push(Row { kind, depth, top: self.height, height });
        self.height += height;
    }

    fn add_layers(&mut self, layers: &[Layer], parent: Option<&str>, depth: usize, folding: &Folding) {
        for (index, l) in layers.iter().enumerate().rev() {
            let open_folder = matches!(&l.kind, LayerKind::Group(c) if !c.is_empty()) && !folding.folded.contains(&l.id);
            let slot = LayerSlot { parent: parent.map(str::to_string), index };
            self.push(RowKind::Layer { id: l.id.clone(), slot, open_folder }, depth, LAYER_ROW);
            if folding.effects_open.contains(&l.id) {
                let mask = l.mask.as_ref().map_or(0, |m| m.effects.len());
                for i in (0..mask).rev() {
                    self.push(RowKind::Effect { layer: l.id.clone(), stack: StackId::Mask, index: i }, depth + 1, EFFECT_ROW);
                }
                for i in (0..l.effects.len()).rev() {
                    self.push(RowKind::Effect { layer: l.id.clone(), stack: StackId::Content, index: i }, depth + 1, EFFECT_ROW);
                }
            }
            if open_folder {
                self.add_layers(l.children(), Some(&l.id), depth + 1, folding);
            }
        }
    }

    /// The rows overlapping `[y0, y1)`, as indices.
    pub fn visible(&self, y0: f32, y1: f32) -> std::ops::Range<usize> {
        let first = self.rows.partition_point(|r| r.top + r.height <= y0);
        let end = self.rows.partition_point(|r| r.top < y1);
        first.min(end)..end
    }

    /// Layer ids in list order (top to bottom), for Shift ranges.
    pub fn layer_order(&self) -> Vec<&str> {
        self.rows
            .iter()
            .filter_map(|r| match &r.kind {
                RowKind::Layer { id, .. } => Some(id.as_str()),
                RowKind::Effect { .. } => None,
            })
            .collect()
    }

    /// The row at height `y` of the list.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        let i = self.rows.partition_point(|r| r.top + r.height <= y);
        self.rows.get(i).filter(|r| y >= r.top).map(|_| i)
    }

    /// Where a drop at height `y` lands; `on_mask` is true when the pointer is over the row's mask thumbnail.
    /// Near a layer row's top or bottom edge the drop goes between rows (marker line), otherwise onto the row:
    /// onto its mask, onto the layer, or for a folder into it (layer moves) or its effects (effects). Effect
    /// rows count as their layer. Below the last row the drop goes to the bottom of the stack.
    pub fn landing(&self, y: f32, on_mask: bool) -> Option<Landing> {
        let Some(i) = self.row_at(y) else {
            let bottom = self.height;
            let at = if y >= bottom { LayerSlot::top(0) } else { return None };
            return Some(Landing { target: DropTarget::Between { set: self.set.clone(), at }, line: Some(bottom), row: None });
        };
        let row = self.rows.get(i)?;
        let between = |at: LayerSlot, line: f32| Some(Landing { target: DropTarget::Between { set: self.set.clone(), at }, line: Some(line), row: None });
        match &row.kind {
            RowKind::Layer { id, slot, open_folder } => {
                let f = (y - row.top) / row.height;
                if f < EDGE {
                    return between(LayerSlot { parent: slot.parent.clone(), index: slot.index + 1 }, row.top);
                }
                if f > 1.0 - EDGE {
                    // Below an open folder's row is the top of its children.
                    let at = if *open_folder { self.top_of_folder(id) } else { slot.clone() };
                    return between(at, row.top + row.height);
                }
                let r = LayerRef::new(self.set.clone(), id.clone());
                let target = if on_mask { DropTarget::OntoMask(r) } else { DropTarget::OntoLayer(r) };
                Some(Landing { target, line: None, row: Some(i) })
            }
            RowKind::Effect { layer, .. } => {
                let owner = self.rows.iter().position(|r| matches!(&r.kind, RowKind::Layer { id, .. } if id == layer));
                Some(Landing { target: DropTarget::OntoLayer(LayerRef::new(self.set.clone(), layer.clone())), line: None, row: owner })
            }
        }
    }

    /// The slot at the top of folder `id`'s children (from the first child row, which is the topmost).
    fn top_of_folder(&self, id: &str) -> LayerSlot {
        let first_child = self.rows.iter().find_map(|r| match &r.kind {
            RowKind::Layer { slot, .. } if slot.parent.as_deref() == Some(id) => Some(slot.index + 1),
            _ => None,
        });
        LayerSlot::in_group(id, first_child.unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::{Channel, PaintContent};

    fn set() -> TextureSet {
        let l = |id: &str| Layer::new(id, id, LayerKind::Paint(PaintContent::default()));
        let mut s = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::Roughness);
        let fx = l("a").with_effect(materialcraft::paint::ContentEffect::new(
            Vec::new(),
            materialcraft::paint::EffectKind::Fill(materialcraft::paint::Fill::uniform(vec![])),
        ));
        s.layers = vec![l("base"), Layer::new("g", "G", LayerKind::Group(vec![fx, l("b")])), l("top")];
        s
    }

    fn ids(rows: &Rows) -> Vec<String> {
        rows.rows
            .iter()
            .map(|r| match &r.kind {
                RowKind::Layer { id, .. } => format!("{}{id}", r.depth),
                RowKind::Effect { layer, index, .. } => format!("{}{layer}.{index}", r.depth),
            })
            .collect()
    }

    #[test]
    fn rows_list_the_stack_top_first_with_folding() {
        let mut folding = Folding::default();
        let rows = Rows::build(&set(), &folding);
        assert_eq!(ids(&rows), ["0top", "0g", "1b", "1a", "0base"]);
        assert_eq!(rows.height, 5.0 * LAYER_ROW);
        folding.effects_open.insert("a".into());
        folding.folded.insert("g".into());
        assert_eq!(ids(&Rows::build(&set(), &folding)), ["0top", "0g", "0base"]);
        folding.folded.clear();
        let rows = Rows::build(&set(), &folding);
        assert_eq!(ids(&rows), ["0top", "0g", "1b", "1a", "2a.0", "0base"]);
        assert_eq!(rows.visible(LAYER_ROW * 1.5, LAYER_ROW * 3.2), 1..4);
        assert_eq!(rows.visible(-10.0, 1.0), 0..1);
        assert_eq!(rows.visible(rows.height, rows.height + 50.0), 6..6);
        assert_eq!(rows.layer_order(), ["top", "g", "b", "a", "base"]);
    }

    #[test]
    fn pointer_heights_become_drop_targets() {
        let mut folding = Folding::default();
        folding.effects_open.insert("a".into());
        let rows = Rows::build(&set(), &folding);
        let between = |parent: Option<&str>, index| DropTarget::Between { set: "s".into(), at: LayerSlot { parent: parent.map(str::to_string), index } };
        let lr = |id: &str| LayerRef::new("s", id);
        let y = |row: usize, f: f32| rows.rows[row].top + rows.rows[row].height * f;
        // Above "top": the very top of the stack.
        assert_eq!(rows.landing(y(0, 0.1), false).unwrap().target, between(None, 3));
        assert_eq!(rows.landing(y(0, 0.1), false).unwrap().line, Some(0.0));
        // Below "top" = above the folder.
        assert_eq!(rows.landing(y(0, 0.9), false).unwrap().target, between(None, 2));
        // Onto the folder, its mask, below its row = top of its children.
        assert_eq!(rows.landing(y(1, 0.5), false).unwrap().target, DropTarget::OntoLayer(lr("g")));
        assert_eq!(rows.landing(y(1, 0.5), true).unwrap().target, DropTarget::OntoMask(lr("g")));
        assert_eq!(rows.landing(y(1, 0.9), false).unwrap().target, between(Some("g"), 2));
        // Between the children, and below the last child.
        assert_eq!(rows.landing(y(2, 0.9), false).unwrap().target, between(Some("g"), 1));
        assert_eq!(rows.landing(y(3, 0.9), false).unwrap().target, between(Some("g"), 0));
        // An effect row counts as its layer.
        let fx = rows.landing(y(4, 0.5), false).unwrap();
        assert_eq!((fx.target, fx.row), (DropTarget::OntoLayer(lr("a")), Some(3)));
        // Under everything: the bottom of the stack.
        assert_eq!(rows.landing(rows.height + 30.0, false).unwrap().target, between(None, 0));
        assert!(rows.landing(-5.0, false).is_none());
    }
}
