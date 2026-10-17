//! [`apply_pick`]: a pick's effect on a polygon fill's selection.

use super::Picked;
use crate::mesh::Mesh;
use crate::paint::effect::{Pick, PickMode, Selection};
use std::collections::{BTreeMap, BTreeSet};

/// Picked triangles by part name, as a selection stores them.
pub type NamedPicks = Vec<(String, BTreeSet<u32>)>;

/// `picked` with each part index replaced by the part's name in `mesh` (so the mesh need not be at hand when the
/// selection is changed); parts the mesh does not have are left out.
pub fn name_picks(mesh: &Mesh, picked: &Picked) -> NamedPicks {
    picked.iter().filter_map(|(pi, tris)| Some((mesh.parts.get(*pi)?.name.clone(), tris.clone()))).collect()
}

/// `selection` with the (grown) `picked` triangles added (`add`, a fill) or taken out (an un-pick). Picks are
/// stored by part name with sorted triangle ids; parts left with nothing are dropped. The selection's mode becomes
/// `mode` (the mode the latest pick was made with). Picks of parts the mesh no longer has are kept.
pub fn apply_pick(selection: &Selection, picked: &NamedPicks, add: bool, mode: PickMode) -> Selection {
    let mut by_part: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in &selection.picks {
        if !by_part.contains_key(&p.part) {
            order.push(p.part.clone());
        }
        by_part.entry(p.part.clone()).or_default().extend(p.triangles.iter().copied());
    }
    for (name, tris) in picked {
        if !by_part.contains_key(name) {
            order.push(name.clone());
        }
        let set = by_part.entry(name.clone()).or_default();
        if add {
            set.extend(tris.iter().copied());
        } else {
            set.retain(|t| !tris.contains(t));
        }
    }
    let picks = order
        .into_iter()
        .filter_map(|name| {
            let tris = by_part.remove(&name)?;
            (!tris.is_empty()).then(|| Pick { part: name, triangles: tris.into_iter().collect() })
        })
        .collect();
    Selection { mode, picks }
}
