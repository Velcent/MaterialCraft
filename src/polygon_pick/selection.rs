//! [`apply_pick`]: a pick's effect on a polygon fill's selection.

use super::Picked;
use crate::mesh::Mesh;
use crate::paint::effect::{Pick, PickMode, Selection};
use std::collections::{BTreeMap, BTreeSet};

/// `selection` with the (grown) `picked` triangles of `mesh` added (`add`, a fill) or taken out (an un-pick).
/// Picks are stored by part name with sorted triangle ids; parts left with nothing are dropped. The selection's
/// mode becomes `mode` (the mode the latest pick was made with). Picks of parts the mesh no longer has are kept.
pub fn apply_pick(selection: &Selection, mesh: &Mesh, picked: &Picked, add: bool, mode: PickMode) -> Selection {
    let mut by_part: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in &selection.picks {
        if !by_part.contains_key(&p.part) {
            order.push(p.part.clone());
        }
        by_part.entry(p.part.clone()).or_default().extend(p.triangles.iter().copied());
    }
    for (pi, tris) in picked {
        let Some(part) = mesh.parts.get(*pi) else { continue };
        if !by_part.contains_key(&part.name) {
            order.push(part.name.clone());
        }
        let set = by_part.entry(part.name.clone()).or_default();
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
