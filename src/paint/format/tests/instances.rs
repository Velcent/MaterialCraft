//! Instance layers (`kind: "instance"` with `instance_of`, since 0.15).

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::json;
use crate::paint::format::FormatError;
use crate::paint::{InstanceLink, Layer, LayerKind};
use crate::zip::ZipReader;
use std::io::Cursor;

fn instance(id: &str, set: &str, layer: &str) -> Layer {
    Layer::new(id, format!("{id} name"), LayerKind::Instance(InstanceLink::new(set, layer)))
}

fn document() -> crate::paint::Document {
    let mut doc = full_document();
    let first = doc.texture_sets[0].layers[0].id.clone();
    doc.texture_sets[1].layers.push(instance("inst", "body", &first));
    let mut hidden = instance("folder-inst", "body", "group");
    hidden.visible = false;
    doc.texture_sets[1].layers.push(Layer::new("holder", "Holder", LayerKind::Group(vec![hidden])));
    // A broken one (it sits in the folder it instances) is saved as it is.
    if let Some(LayerKind::Group(children)) = doc.texture_sets[0].layer_mut("group").map(|g| &mut g.kind) {
        children.push(instance("loop", "body", "group"));
    }
    doc
}

#[test]
fn instances_round_trip() {
    let doc = document();
    assert!(doc.instance_on_cycle("body", "loop"));
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back, doc);
    assert_eq!(to_bytes(&back), bytes, "deterministic");
    let mut zip = ZipReader::open(Cursor::new(&bytes)).unwrap();
    let text = String::from_utf8(zip.read("document.json").unwrap()).unwrap();
    let v = json::parse(&text).unwrap();
    let layer = &v.get("texture_sets").unwrap().as_array().unwrap()[1].get("layers").unwrap().as_array().unwrap()[0];
    assert_eq!(layer.get("kind").and_then(|k| k.as_str()), Some("instance"));
    let link = layer.get("instance_of").unwrap();
    assert_eq!(link.get("set").and_then(|s| s.as_str()), Some("body"));
    assert!(layer.get("images").is_none() && layer.get("children").is_none());
}

#[test]
fn an_instance_without_its_link_is_an_error() {
    let bytes = to_bytes(&document());
    let broken = rewrite(&bytes, |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        Some(String::from_utf8(data).unwrap().replacen("\"instance_of\"", "\"instance_off\"", 1).into_bytes())
    });
    assert!(matches!(from_bytes(&broken), Err(FormatError::Manifest { .. })));
}
