//! [`Revisions`]: a revision number for every layer of a texture set, kept across builds. Each build compares the
//! layers with copies of them as last seen (without their children; images share their tiles, so equal tiles
//! compare by pointer); a layer that differs gets a new number. From those, per layer, a hash of the layers before
//! it in compositing order ([`SetRevs::prefix`]) and of its subtree ([`SetRevs::deep`]).

use crate::paint::channel::Channel;
use crate::paint::layer::{Layer, LayerKind};
use crate::paint::texture_set::{BakedMap, TextureSet};
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Default)]
pub(super) struct Revisions {
    /// Each layer as last seen (without children) and its revision.
    seen: HashMap<String, (Layer, u64)>,
    baked: Vec<BakedMap>,
    baked_rev: u64,
    next: u64,
    /// The top-level layers (id, subtree hash) of the last build.
    top: Vec<(String, u64)>,
    /// The top-level layer that changed last.
    hot: Option<usize>,
}

/// The revisions of one build.
#[derive(Debug, Default)]
pub(super) struct SetRevs {
    pub own: HashMap<String, u64>,
    /// Hash of every layer before the layer in pre-order (folders before their children): a superset of the stack
    /// below it.
    pub prefix: HashMap<String, u64>,
    /// Like `prefix`, counting only what the layers before do to the height (generators read the height below).
    pub height_prefix: HashMap<String, u64>,
    /// Hash of the layer and its children.
    pub deep: HashMap<String, u64>,
    /// Revision of the set's baked maps.
    pub baked: u64,
    /// The top-level layer (by index) that changed last: the first that differed from the build before.
    pub first_changed: Option<usize>,
}

impl Revisions {
    /// The set's revisions now; `None` if two layers share an id (then nothing is cached).
    pub fn observe(&mut self, set: &TextureSet) -> Option<SetRevs> {
        let mut out = SetRevs::default();
        let (mut running, mut height) = (0u64, 0u64);
        for (_, l) in set.walk_layers() {
            let rev = match self.seen.get(&l.id) {
                Some((s, r)) if s.same_without_children(l) => *r,
                _ => {
                    self.next += 1;
                    self.seen.insert(l.id.clone(), (l.without_children(), self.next));
                    self.next
                }
            };
            if out.own.insert(l.id.clone(), rev).is_some() {
                return None;
            }
            out.prefix.insert(l.id.clone(), running);
            out.height_prefix.insert(l.id.clone(), height);
            running = hash(&(running, &l.id, rev));
            // a layer without a height entry leaves the height alone whatever else changes in it
            let writes_height = l.blend_for(Channel::Height).is_some();
            height = hash(&(height, &l.id, writes_height.then_some(rev)));
        }
        self.seen.retain(|id, _| out.own.contains_key(id));
        for l in &set.layers {
            deep(l, &mut out);
        }
        if self.baked != set.baked {
            self.next += 1;
            self.baked = set.baked.clone();
            self.baked_rev = self.next;
        }
        out.baked = self.baked_rev;
        let top: Vec<(String, u64)> = set.layers.iter().map(|l| (l.id.clone(), out.deep.get(&l.id).copied().unwrap_or(0))).collect();
        if !self.top.is_empty() {
            let changed = top.iter().zip(&self.top).position(|(a, b)| a != b).or((top.len() != self.top.len()).then_some(top.len().min(self.top.len())));
            self.hot = changed.or(self.hot);
        }
        out.first_changed = self.hot;
        self.top = top;
        Some(out)
    }
}

fn deep(l: &Layer, revs: &mut SetRevs) -> u64 {
    let own = revs.own.get(&l.id).copied().unwrap_or(0);
    let children: Vec<u64> = match &l.kind {
        LayerKind::Group(c) => c.iter().map(|c| deep(c, revs)).collect(),
        _ => Vec::new(),
    };
    let h = hash(&(own, children));
    revs.deep.insert(l.id.clone(), h);
    h
}

pub(super) fn hash<T: Hash>(v: &T) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
