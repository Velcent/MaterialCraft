//! The layers a dropped material or smart material makes (LS-18): a material becomes a fill layer whose fill is
//! the material ([`FillSource::Material`]), a smart material a folder holding its layers. Where they go:
//!
//! - **between layers**: at that slot;
//! - **onto a folder**: on top of its children;
//! - **onto another layer**: directly above it, except a material dropped onto a fill layer, which becomes that
//!   layer's fill (its projection is kept).

use super::super::ids::IdPool;
use super::super::{Edit, EditError, LayerProperty, LayerSlot, Transaction};
use super::drop::{DropOutcome, DropTarget};
use super::{channels, layer, set, slot_of};
use crate::paint::blend::BlendMode;
use crate::paint::fragment::adapt;
use crate::paint::layer::{Fill, FillSource, Layer, LayerKind};
use crate::paint::texture_set::TextureSet;

/// A material as an asset: the reference a fill stores (resolved later through the material lookup folders:
/// the document's folder and the asset library roots) and parameter overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialAsset {
    /// Shown as the new fill layer's name.
    pub name: String,
    /// The material reference, e.g. `materials/rust.mca`.
    pub path: String,
    pub params: Vec<(String, String)>,
}

impl MaterialAsset {
    fn source(&self) -> FillSource {
        FillSource::Material { path: self.path.clone(), params: self.params.clone() }
    }
}

/// Drops a material: a new fill layer, or the fill of the fill layer it lands on.
pub(super) fn material(tx: &mut Transaction, target: &DropTarget, asset: &MaterialAsset) -> Result<DropOutcome, EditError> {
    if let DropTarget::OntoLayer(r) = target
        && let LayerKind::Fill(fill) = &layer(tx, r)?.kind
    {
        let fill = Fill { source: asset.source(), projection: fill.projection.clone(), original: fill.original.clone() };
        tx.apply(Edit::SetLayer { layer: r.clone(), property: LayerProperty::Fill(fill) })?;
        return Ok(DropOutcome::Filled(r.clone()));
    }
    let s = set(tx, target.set())?;
    let at = slot_for(s, target)?;
    let id = IdPool::of(s).layer("fill");
    let name = if asset.name.is_empty() { "Material".to_string() } else { asset.name.clone() };
    let layer = with_every_channel(s, Layer::new(id.clone(), name, LayerKind::Fill(Fill::new(asset.source()))));
    tx.apply(Edit::InsertLayer { set: target.set().to_string(), at, layer: Box::new(layer) })?;
    Ok(DropOutcome::NewLayer(id))
}

/// Drops a smart material: a folder named `name` with copies of `layers` under ids new in the texture set, fitted
/// to the set (resolution, declared channels), recording `asset` as its provenance.
pub(super) fn folder(tx: &mut Transaction, target: &DropTarget, name: &str, layers: &[Layer], asset: Option<&str>) -> Result<DropOutcome, EditError> {
    let s = set(tx, target.set())?;
    let at = slot_for(s, target)?;
    let mut ids = IdPool::of(s);
    let name = if name.is_empty() { "Smart material".to_string() } else { name.to_string() };
    let mut children = layers.to_vec();
    adapt::fit_layers(&mut children, s);
    let mut group = with_every_channel(s, Layer::new("folder", name, LayerKind::Group(children)));
    group.asset = asset.map(str::to_string);
    ids.reid(&mut group);
    let id = group.id.clone();
    tx.apply(Edit::InsertLayer { set: target.set().to_string(), at, layer: Box::new(group) })?;
    Ok(DropOutcome::NewLayer(id))
}

/// Where a new layer goes for `target` (see the module docs).
fn slot_for(set: &TextureSet, target: &DropTarget) -> Result<LayerSlot, EditError> {
    match target {
        DropTarget::Between { at, .. } => Ok(at.clone()),
        DropTarget::OntoLayer(r) => {
            let l = set.layer(&r.layer).ok_or_else(|| EditError::NoLayer { set: r.set.clone(), layer: r.layer.clone() })?;
            match &l.kind {
                LayerKind::Group(children) => Ok(LayerSlot::in_group(r.layer.clone(), children.len())),
                _ => {
                    let slot = slot_of(set, &r.layer).ok_or_else(|| EditError::NoLayer { set: r.set.clone(), layer: r.layer.clone() })?;
                    Ok(LayerSlot { parent: slot.parent, index: slot.index + 1 })
                }
            }
        }
        DropTarget::OntoMask(_) => Err(EditError::NotAccepted("a material goes between layers or onto a layer")),
    }
}

/// The layer affecting every channel of `set` with `Normal` at full opacity (as a new layer does).
fn with_every_channel(set: &TextureSet, mut layer: Layer) -> Layer {
    for c in channels(set) {
        layer.set_blend(c, BlendMode::Normal, 1.0);
    }
    layer
}

#[cfg(test)]
mod tests {
    use super::super::super::{LayerRef, Transaction};
    use super::super::{DropPayload, accepts, drop_onto};
    use super::*;
    use crate::paint::channel::Channel;
    use crate::paint::doc::Document;
    use crate::paint::layer::PaintContent;

    fn doc() -> Document {
        let l = |id: &str| Layer::new(id, id, LayerKind::Paint(PaintContent::default()));
        let mut s = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
        let fill = Layer::new("f", "F", LayerKind::Fill(Fill::uniform(vec![(Channel::Roughness, [0.5; 4])])));
        s.layers = vec![l("base"), Layer::new("g", "G", LayerKind::Group(vec![l("a")])), fill];
        let mut d = Document::default();
        d.texture_sets.push(s);
        d
    }

    fn rust() -> DropPayload {
        DropPayload::Material(MaterialAsset { name: "Rust".into(), path: "materials/rust.mca".into(), params: vec![("scale".into(), "2".into())] })
    }

    fn drop(d: &mut Document, target: DropTarget, payload: &DropPayload) -> DropOutcome {
        assert!(accepts(d, &target, payload), "{target:?}");
        let mut tx = Transaction::new(d);
        let out = drop_onto(&mut tx, &target, payload, false).unwrap();
        tx.commit();
        d.validate().unwrap();
        out
    }

    fn top_ids(d: &Document) -> Vec<String> {
        d.texture_sets[0].layers.iter().map(|l| l.id.clone()).collect()
    }

    #[test]
    fn a_material_between_layers_becomes_a_fill_layer_with_the_material() {
        let mut d = doc();
        let out = drop(&mut d, DropTarget::Between { set: "s".into(), at: LayerSlot::top(1) }, &rust());
        assert_eq!(out, DropOutcome::NewLayer("fill1".into()));
        assert_eq!(top_ids(&d), ["base", "fill1", "g", "f"]);
        let l = d.texture_sets[0].layer("fill1").unwrap();
        assert_eq!(l.name, "Rust");
        assert_eq!(l.blend.len(), 2);
        assert!(
            matches!(&l.kind, LayerKind::Fill(f) if f.source == FillSource::Material { path: "materials/rust.mca".into(), params: vec![("scale".into(), "2".into())] })
        );
    }

    #[test]
    fn a_material_onto_layers_goes_above_into_folders_and_into_fill_layers() {
        let mut d = doc();
        drop(&mut d, DropTarget::OntoLayer(LayerRef::new("s", "base")), &rust());
        assert_eq!(top_ids(&d), ["base", "fill1", "g", "f"]);
        drop(&mut d, DropTarget::OntoLayer(LayerRef::new("s", "g")), &rust());
        assert_eq!(d.texture_sets[0].layer("g").unwrap().children().iter().map(|l| l.id.as_str()).collect::<Vec<_>>(), ["a", "fill2"]);
        let out = drop(&mut d, DropTarget::OntoLayer(LayerRef::new("s", "f")), &rust());
        assert_eq!(out, DropOutcome::Filled(LayerRef::new("s", "f")));
        assert!(matches!(&d.texture_sets[0].layer("f").unwrap().kind, LayerKind::Fill(f) if matches!(f.source, FillSource::Material { .. })));
        assert!(!accepts(&d, &DropTarget::OntoMask(LayerRef::new("s", "a")), &rust()));
    }

    #[test]
    fn a_smart_material_becomes_a_folder_with_fresh_ids() {
        let mut d = doc();
        let inner = vec![Layer::new("a", "Dirt", LayerKind::Paint(PaintContent::default())), Layer::new("f", "Base", LayerKind::Fill(Fill::uniform(vec![])))];
        let embed = vec![crate::paint::EmbeddedAsset::new("smart-materials/worn.mcfrag", vec![1, 2])];
        let payload = DropPayload::Folder { name: "Worn paint".into(), layers: inner, asset: Some("smart-materials/worn.mcfrag".into()), embed };
        let out = drop(&mut d, DropTarget::Between { set: "s".into(), at: LayerSlot::top(3) }, &payload);
        let DropOutcome::NewLayer(id) = out else { panic!("{out:?}") };
        let folder = d.texture_sets[0].layer(&id).unwrap();
        assert_eq!(folder.name, "Worn paint");
        assert_eq!(folder.asset.as_deref(), Some("smart-materials/worn.mcfrag"));
        assert_eq!(d.assets.len(), 1, "the smart material is embedded with its folder");
        let kids: Vec<&str> = folder.children().iter().map(|l| l.id.as_str()).collect();
        assert_eq!(kids.len(), 2);
        assert!(!kids.contains(&"a") && !kids.contains(&"f"), "ids are unique in the set: {kids:?}");
    }
}
