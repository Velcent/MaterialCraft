//! The edit mode's decisions: what is hidden, which event a click or a tick makes.

use super::*;
use materialcraft::mesh::{Mesh, MeshPart};
use materialcraft::paint::{LayerKind, PaintContent};

/// Parts "body" and "trim" on slot 0 (the set), "glass" on slot 1.
fn doc(mask: Option<GeometryMask>) -> Document {
    let part = |name: &str, material: usize| MeshPart { name: name.into(), material, ..MeshPart::default() };
    let mut doc = Document {
        mesh: Mesh { materials: vec!["a".into(), "b".into()], parts: vec![part("body", 0), part("glass", 1), part("trim", 0)] },
        ..Document::default()
    };
    let mut set = TextureSet::new("s", "S", (8, 8));
    let mut layer = Layer::new("l", "L", LayerKind::Paint(PaintContent::default()));
    layer.geometry_mask = mask;
    set.layers.push(layer);
    doc.texture_sets.push(set);
    doc
}

#[test]
fn excluded_parts_of_the_set_are_hidden_and_clicks_exclude() {
    let r = LayerRef::new("s", "l");
    let d = doc(None);
    let t = MaskTarget::find(&d, &r).unwrap();
    assert_eq!(t.parts(), ["body", "trim"]);
    assert_eq!(t.hidden_parts(), [false, false, false]);
    assert_eq!(t.exclude_mesh_part(1), None, "another set's part is not this mask's");
    assert_eq!(t.exclude_mesh_part(2), Some(LayerEvent::SetGeometryMask(r.clone(), Some(GeometryMask::Parts(vec!["body".into()])))));
    assert_eq!(t.replace(None), None, "including all of an unmasked layer changes nothing");

    let d = doc(Some(GeometryMask::Parts(vec!["body".into()])));
    let t = MaskTarget::find(&d, &r).unwrap();
    assert_eq!(t.hidden_parts(), [false, false, true], "only the set's excluded part is hidden");
    assert_eq!(t.set_part("trim", true), Some(LayerEvent::SetGeometryMask(r.clone(), None)), "all parts back is no mask");
    assert_eq!(t.set_part("trim", false), None);
    assert!(MaskTarget::find(&d, &LayerRef::new("s", "gone")).is_none());
}
