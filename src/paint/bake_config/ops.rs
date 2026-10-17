//! The bakers panel's operations as pure document operations: each plans [`Edit`]s on the document inside a
//! [`Transaction`], so the panel's changes are undoable steps and run headless in tests.
//!
//! - Settings: [`set_common`] and [`set_map_settings`] change one set, or every set while that part is linked
//!   ([`super::BakeLinks`]); [`set_link`] links a part and makes every set take the given set's values.
//! - One-off copies: [`copy_part`] and [`copy_all`] to chosen sets.
//! - Check states: [`set_flags`], [`show_map`], [`check_all`], [`invert_checks`], [`copy_checks`].
//! - Results: [`store_baked`] puts a bake's maps into the set's baked maps.
//!
//! Operations that change nothing make no edit (no empty history steps).

use super::config::{CommonBake, MapFlags, MapSettings, SetBakeConfig};
use super::links::BakePart;
use crate::bake::MapKind;
use crate::paint::edit::{Edit, EditError, Transaction};
use crate::paint::texture_set::BakedMap;

fn config(tx: &Transaction, set: &str) -> Result<SetBakeConfig, EditError> {
    tx.doc().texture_set(set).map(|s| s.bake.clone()).ok_or_else(|| EditError::NoTextureSet(set.to_string()))
}

/// Applies `change` to the configuration of `set`; no edit when it changes nothing.
fn update(tx: &mut Transaction, set: &str, change: impl FnOnce(&mut SetBakeConfig)) -> Result<(), EditError> {
    let before = config(tx, set)?;
    let mut after = before.clone();
    change(&mut after);
    if after == before {
        return Ok(());
    }
    tx.apply(Edit::SetBakeConfig { set: set.to_string(), config: Box::new(after) })
}

/// `set`, or every set while `part` is linked.
fn affected(tx: &Transaction, set: &str, part: BakePart) -> Result<Vec<String>, EditError> {
    if tx.doc().texture_set(set).is_none() {
        return Err(EditError::NoTextureSet(set.to_string()));
    }
    Ok(if tx.doc().bake_links.is_linked(part) { all_sets(tx) } else { vec![set.to_string()] })
}

fn all_sets(tx: &Transaction) -> Vec<String> {
    tx.doc().texture_sets.iter().map(|s| s.id.clone()).collect()
}

/// Makes `part` of `to` equal to `from`'s.
fn copy_into(target: &mut SetBakeConfig, from: &SetBakeConfig, part: BakePart) {
    match part {
        BakePart::Common => target.common = from.common.clone(),
        BakePart::Map(kind) => target.maps.set_settings(from.maps.get(kind).settings().clone()),
    }
}

/// Sets the common settings of `set` (of every set while they are linked).
pub fn set_common(tx: &mut Transaction, set: &str, common: &CommonBake) -> Result<(), EditError> {
    for id in affected(tx, set, BakePart::Common)? {
        update(tx, &id, |c| c.common = common.clone())?;
    }
    Ok(())
}

/// Sets the settings of the map they belong to in `set` (in every set while that map is linked).
pub fn set_map_settings(tx: &mut Transaction, set: &str, settings: &MapSettings) -> Result<(), EditError> {
    for id in affected(tx, set, BakePart::Map(settings.kind()))? {
        update(tx, &id, |c| c.maps.set_settings(settings.clone()))?;
    }
    Ok(())
}

/// Links or unlinks `part`. Linking makes every set take `from`'s values of that part.
pub fn set_link(tx: &mut Transaction, from: &str, part: BakePart, on: bool) -> Result<(), EditError> {
    let source = config(tx, from)?;
    let links = tx.doc().bake_links.with(part, on);
    if links != tx.doc().bake_links {
        tx.apply(Edit::SetBakeLinks(links))?;
    }
    if on {
        for id in all_sets(tx) {
            update(tx, &id, |c| copy_into(c, &source, part))?;
        }
    }
    Ok(())
}

/// Copies `part` of `from` into each set of `to` (once; nothing stays linked).
pub fn copy_part(tx: &mut Transaction, from: &str, part: BakePart, to: &[String]) -> Result<(), EditError> {
    let source = config(tx, from)?;
    for id in to.iter().filter(|id| *id != from) {
        update(tx, id, |c| copy_into(c, &source, part))?;
    }
    Ok(())
}

/// Copies every setting of `from` (common and every map's settings, not the check states) into each set of `to`.
pub fn copy_all(tx: &mut Transaction, from: &str, to: &[String]) -> Result<(), EditError> {
    let source = config(tx, from)?;
    for id in to.iter().filter(|id| *id != from) {
        update(tx, id, |c| {
            copy_into(c, &source, BakePart::Common);
            MapKind::ALL.into_iter().for_each(|k| copy_into(c, &source, BakePart::Map(k)));
        })?;
    }
    Ok(())
}

/// Sets the flags of one map of `set`. Showing a map hides the set's other maps (one map is shown at a time).
pub fn set_flags(tx: &mut Transaction, set: &str, kind: MapKind, flags: MapFlags) -> Result<(), EditError> {
    update(tx, set, |c| {
        if flags.show {
            MapKind::ALL.into_iter().for_each(|k| c.maps.flags_mut(k).show = false);
        }
        *c.maps.flags_mut(kind) = flags;
    })
}

/// Shows `kind` of `set` in the viewport, or (`None`) none of its maps.
pub fn show_map(tx: &mut Transaction, set: &str, kind: Option<MapKind>) -> Result<(), EditError> {
    update(tx, set, |c| MapKind::ALL.into_iter().for_each(|k| c.maps.flags_mut(k).show = Some(k) == kind))
}

/// Includes every map of `set` in its bake (`on`), or none.
pub fn check_all(tx: &mut Transaction, set: &str, on: bool) -> Result<(), EditError> {
    update(tx, set, |c| MapKind::ALL.into_iter().for_each(|k| c.maps.flags_mut(k).include = on))
}

/// Includes the maps of `set` that were left out and leaves out the others.
pub fn invert_checks(tx: &mut Transaction, set: &str) -> Result<(), EditError> {
    update(tx, set, |c| {
        MapKind::ALL.into_iter().for_each(|k| {
            let f = c.maps.flags_mut(k);
            f.include = !f.include;
        })
    })
}

/// Gives each set of `to` the same included maps as `from`.
pub fn copy_checks(tx: &mut Transaction, from: &str, to: &[String]) -> Result<(), EditError> {
    let included = config(tx, from)?.included();
    for id in to.iter().filter(|id| *id != from) {
        update(tx, id, |c| MapKind::ALL.into_iter().for_each(|k| c.maps.flags_mut(k).include = included.contains(k)))?;
    }
    Ok(())
}

/// Puts freshly baked `maps` into `set`'s baked maps: a map of a kind the set has replaces it, others are added.
pub fn store_baked(tx: &mut Transaction, set: &str, maps: Vec<BakedMap>) -> Result<(), EditError> {
    let mut baked = tx.doc().texture_set(set).map(|s| s.baked.clone()).ok_or_else(|| EditError::NoTextureSet(set.to_string()))?;
    for map in maps {
        match baked.iter_mut().find(|b| b.kind == map.kind) {
            Some(old) => *old = map,
            None => baked.push(map),
        }
    }
    tx.apply(Edit::SetBaked { set: set.to_string(), maps: baked })
}
