//! Converting layouts between a screen's panel type and the id strings the settings store.

use craft_ui::docking::{Floating, Layout, Location, MAX_DEPTH, Node, Placement, StackEntry};

/// `layout` with every panel mapped by `f`; `None` if `f` refuses one or the tree is deeper than
/// the docking model allows (a hand-edited file can't make this recurse without bound).
pub fn map_layout<A, B>(layout: &Layout<A>, f: &impl Fn(&A) -> Option<B>) -> Option<Layout<B>> {
    let root = match &layout.root {
        Some(n) => Some(map_node(n, f, 0)?),
        None => None,
    };
    let floating = layout
        .floating
        .iter()
        .map(|g| Some(Floating { panels: g.panels.iter().map(f).collect::<Option<Vec<B>>>()?, active: g.active, rect: g.rect }))
        .collect::<Option<Vec<_>>>()?;
    Some(Layout { root, floating })
}

fn map_node<A, B>(node: &Node<A>, f: &impl Fn(&A) -> Option<B>, depth: usize) -> Option<Node<B>> {
    if depth > MAX_DEPTH {
        return None;
    }
    Some(match node {
        Node::Split { axis, size, first, second } => {
            Node::Split { axis: *axis, size: *size, first: Box::new(map_node(first, f, depth + 1)?), second: Box::new(map_node(second, f, depth + 1)?) }
        }
        Node::Tabs { panels, active } => Node::Tabs { panels: panels.iter().map(f).collect::<Option<Vec<B>>>()?, active: *active },
        Node::Stack { entries } => Node::Stack {
            entries: entries.iter().map(|e| Some(StackEntry { panel: f(&e.panel)?, open: e.open, height: e.height })).collect::<Option<Vec<_>>>()?,
        },
    })
}

/// A closed panel's location with its panels mapped by `f`.
pub fn map_location<A, B>(loc: &Location<A>, f: &impl Fn(&A) -> Option<B>) -> Option<Location<B>> {
    let anchor = match &loc.anchor {
        Some(a) => Some(f(a)?),
        None => None,
    };
    let placement = match &loc.placement {
        Placement::Tab { before: Some(b) } => Placement::Tab { before: Some(f(b)?) },
        Placement::Tab { before: None } => Placement::Tab { before: None },
        Placement::Split(z) => Placement::Split(*z),
    };
    Some(Location {
        floating: loc.floating,
        anchor,
        placement,
        size: loc.size,
        stack: loc.stack,
        sibling: loc.sibling.iter().map(f).collect::<Option<Vec<B>>>()?,
    })
}
