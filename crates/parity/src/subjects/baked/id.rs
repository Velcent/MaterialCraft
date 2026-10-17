//! [`IdMap`]: Colour/ID map (ours: one colour per OBJ group or mesh part).
//! Painter colours by its own source (vertex colour, material, ...) and ours picks arbitrary colours per group,
//! so only the region layout is comparable: before measuring, each of our colours is replaced by the reference
//! colour it covers most ([`IdMap::align`]). What remains is where the regions disagree.

use super::{NEEDS, baked_map};
use crate::inputs::Inputs;
use crate::settings::Params;
use crate::subject::{CaseError, Component, Need, RGB, ReferenceKind, Subject};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;
use std::collections::HashMap;

pub struct IdMap;

impl Subject for IdMap {
    fn id(&self) -> &'static str {
        "baked.id"
    }

    fn description(&self) -> &'static str {
        "Colour/ID map, compared by region layout (our colours mapped onto Painter's)"
    }

    fn reference(&self) -> ReferenceKind {
        ReferenceKind::Baked(BakedKind::Id)
    }

    fn needs(&self) -> &'static [Need] {
        NEEDS
    }

    fn components(&self) -> &'static [Component] {
        RGB
    }

    fn run(&self, inputs: &Inputs, params: &Params) -> Result<Tex, CaseError> {
        baked_map(inputs, params, "id")
    }

    fn align(&self, ours: Tex, theirs: &Tex, mask: &[bool]) -> Tex {
        // votes: our colour -> (their colour -> texels)
        let mut votes: HashMap<Key, HashMap<Key, usize>> = HashMap::new();
        for ((o, t), _) in ours.px.iter().zip(&theirs.px).zip(mask).filter(|(_, m)| **m) {
            *votes.entry(key(o)).or_default().entry(key(t)).or_default() += 1;
        }
        let map: HashMap<Key, [f32; 4]> = votes
            .into_iter()
            .filter_map(|(o, t)| t.into_iter().max_by_key(|(k, n)| (*n, *k)).map(|(k, _)| (o, unkey(k))))
            .collect();
        ours.map(|p| map.get(&key(&p)).copied().unwrap_or(p))
    }
}

/// A colour quantised to 8 bits per channel (RGB).
type Key = [u8; 3];

fn key(p: &[f32; 4]) -> Key {
    [0, 1, 2].map(|c| (p[c].clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn unkey(k: Key) -> [f32; 4] {
    [
        f32::from(k[0]) / 255.0,
        f32::from(k[1]) / 255.0,
        f32::from(k[2]) / 255.0,
        1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_our_regions_onto_theirs() {
        let (a, b) = ([0.1, 0.2, 0.3, 1.0], [0.9, 0.8, 0.7, 1.0]);
        let (x, y) = ([1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]);
        let ours = Tex {
            w: 4,
            h: 1,
            px: vec![a, a, b, b],
        };
        let theirs = Tex {
            w: 4,
            h: 1,
            px: vec![x, x, x, y],
        };
        let aligned = IdMap.align(ours, &theirs, &[true; 4]);
        assert_eq!(
            aligned.px,
            vec![x, x, x, x],
            "b covers x and y once each; ties go to the larger key"
        );
    }
}
