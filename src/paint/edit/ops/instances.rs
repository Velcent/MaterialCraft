//! Making instances (IN-05, IN-07): "Paste as instance" and "Instantiate across texture sets". An instance links to
//! the source's own source when the source is itself an instance, so chains stay one step long.

use super::super::ids::IdPool;
use super::super::{Edit, EditError, LayerRef, LayerSlot, Transaction};
use super::{layer, set, slot_of};
use crate::paint::instance::InstanceLink;
use crate::paint::layer::{Layer, LayerKind};

/// The link a new instance of `source` gets, and the name it starts with.
fn link_to(tx: &Transaction, source: &LayerRef) -> Result<(InstanceLink, String), EditError> {
    let l = layer(tx, source)?;
    let link = match &l.kind {
        LayerKind::Instance(link) => {
            tx.doc().instance_source(link).ok_or(EditError::NotAccepted("that instance is broken: its source is gone or it depends on itself"))?
        }
        _ => InstanceLink::new(source.set.clone(), source.layer.clone()),
    };
    Ok((link, l.name.clone()))
}

fn insert(tx: &mut Transaction, set_id: &str, at: LayerSlot, link: InstanceLink, name: &str) -> Result<String, EditError> {
    let id = IdPool::of(set(tx, set_id)?).layer("instance");
    let instance = Layer::new(id.clone(), name, LayerKind::Instance(link));
    tx.apply(Edit::InsertLayer { set: set_id.to_string(), at, layer: Box::new(instance) })?;
    Ok(id)
}

/// The slot directly above `id` in `set_id`, or the top of the stack.
fn above(tx: &Transaction, set_id: &str, id: Option<&str>) -> Result<LayerSlot, EditError> {
    let s = set(tx, set_id)?;
    Ok(match id.and_then(|id| slot_of(s, id)) {
        Some(at) => LayerSlot { parent: at.parent, index: at.index + 1 },
        None => LayerSlot::top(s.layers.len()),
    })
}

/// Paste as instance (IN-05): an instance of `source` in set `set_id`, directly above layer `above_of` (the top of
/// the stack without one); returns its id. Refused where it would depend on itself (inside the folder it
/// instances, IN-11).
pub fn paste_instance(tx: &mut Transaction, source: &LayerRef, set_id: &str, above_of: Option<&str>) -> Result<String, EditError> {
    let (link, name) = link_to(tx, source)?;
    let at = above(tx, set_id, above_of)?;
    insert(tx, set_id, at, link, &name)
}

/// Instantiate across texture sets (IN-07): an instance of `source` in each of `sets` (ids), in the source's own set
/// directly above it, elsewhere at the top of the stack; returns them in the order of `sets`. All or nothing.
pub fn instantiate(tx: &mut Transaction, source: &LayerRef, sets: &[String]) -> Result<Vec<LayerRef>, EditError> {
    if sets.is_empty() {
        return Err(EditError::NotAccepted("no texture set is chosen"));
    }
    let (link, name) = link_to(tx, source)?;
    let mut out = Vec::with_capacity(sets.len());
    for set_id in sets {
        let next_to = (*set_id == source.set).then_some(source.layer.as_str());
        let at = above(tx, set_id, next_to)?;
        let id = insert(tx, set_id, at, link.clone(), &name)?;
        out.push(LayerRef::new(set_id.clone(), id));
    }
    Ok(out)
}
