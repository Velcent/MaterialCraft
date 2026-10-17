//! Fragments (`.mcfrag`, since 0.6): smart materials and smart masks round trip losslessly and byte-identically,
//! their header reads on its own, and documents embed library assets under `resources/assets/`.

use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::paint::format::fragment::{self, FragmentInfo};
use crate::paint::format::strokes::tests::sample_record;
use crate::paint::format::{FormatError, SaveOptions, Thumbnail};
use crate::paint::stroke::{StrokeData, StrokeOwner};
use crate::paint::{BakedKind, EmbeddedAsset, Fragment, FragmentKind, Layer, LayerKind};
use crate::zip::ZipReader;
use std::io::Cursor;

/// Every layer of the full document's body set (paint, fill, material, image masks, groups, every effect kind,
/// an unknown effect kind, geometry masks) with stroke records, as a smart material.
fn smart_material() -> Fragment {
    let mut doc = full_document();
    let set = &mut doc.texture_sets[0];
    for (layer, owner) in [("p1", StrokeOwner::Paint), ("effects", StrokeOwner::Mask { effect: 0 })] {
        *set.layer_mut(layer).unwrap().strokes_mut(owner).unwrap() = Some(StrokeData::Record(sample_record(3)));
    }
    let mut f = Fragment::smart_material("Worn paint", set, set.layers.clone());
    f.needs = vec![BakedKind::Curvature, BakedKind::Other("bent".into())];
    f.assets = vec![EmbeddedAsset::new("materials/rust.mca", b"graph text".to_vec())];
    f
}

fn smart_mask() -> Fragment {
    let doc = full_document();
    let set = &doc.texture_sets[0];
    let mask = set.layer("effects").unwrap().mask.clone().unwrap();
    let mut f = Fragment::smart_mask("Edges", set, mask);
    f.needs = vec![BakedKind::AmbientOcclusion];
    f
}

fn save(f: &Fragment) -> Vec<u8> {
    let mut out = Vec::new();
    fragment::save(f, &mut out).unwrap();
    out
}

#[test]
fn fragments_round_trip_losslessly_and_byte_identically() {
    for f in [smart_material(), smart_mask()] {
        let bytes = save(&f);
        let back = fragment::load(Cursor::new(&bytes)).unwrap();
        assert_eq!(back, f, "{}", f.name);
        assert_eq!(save(&back), bytes, "{}: same fragment, same bytes", f.name);
        let entries: Vec<String> = ZipReader::open(Cursor::new(&bytes)).unwrap().entries().iter().map(|e| e.name.clone()).collect();
        assert_eq!(entries.first().map(String::as_str), Some(fragment::MANIFEST));
        assert!(!entries.iter().any(|e| e.starts_with("mesh/")), "no mesh: {entries:?}");
    }
}

#[test]
fn the_header_reads_alone_and_a_smart_material_has_a_thumbnail() {
    let bytes = save(&smart_material());
    let info = fragment::read_info(Cursor::new(&bytes)).unwrap();
    assert_eq!(
        info,
        FragmentInfo {
            kind: FragmentKind::SmartMaterial,
            name: "Worn paint".into(),
            resolution: (150, 130),
            needs: vec![BakedKind::Curvature, BakedKind::Other("bent".into())],
            thumbnail: Some("thumbnail.png".into()),
        }
    );
    assert!(ZipReader::open(Cursor::new(&bytes)).unwrap().find("resources/assets/materials/rust.mca").is_some());
    let mut plain = Vec::new();
    fragment::save_with(&smart_material(), &mut plain, &SaveOptions { thumbnail: Thumbnail::None }).unwrap();
    assert_eq!(fragment::read_info(Cursor::new(&plain)).unwrap().thumbnail, None);
    assert_eq!(fragment::read_info(Cursor::new(save(&smart_mask()))).unwrap().kind, FragmentKind::SmartMask);
}

#[test]
fn documents_are_not_fragments_and_broken_fragments_are_refused() {
    let doc = to_bytes(&full_document());
    assert!(matches!(fragment::load(Cursor::new(&doc)), Err(FormatError::MissingEntry(_))));
    let mut f = smart_material();
    if let crate::paint::FragmentContent::Layers(layers) = &mut f.content {
        layers.push(Layer::new("p1", "duplicate id", LayerKind::Group(vec![])));
    }
    assert!(matches!(fragment::save(&f, &mut Vec::new()), Err(FormatError::Invalid(_))));
    let mut f = smart_mask();
    f.assets.push(EmbeddedAsset::new("../escape.mca", vec![]));
    assert!(matches!(fragment::save(&f, &mut Vec::new()), Err(FormatError::Unsupported(_))));
}

#[test]
fn documents_embed_assets_and_folders_record_their_smart_material() {
    let mut doc = full_document();
    doc.assets = vec![EmbeddedAsset::new("materials/rust.mca", b"graph".to_vec()), EmbeddedAsset::new("smart-materials/worn.mcfrag", save(&smart_mask()))];
    doc.texture_sets[0].layer_mut("group").unwrap().asset = Some("smart-materials/worn.mcfrag".into());
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back, doc);
    assert_eq!(to_bytes(&back), bytes);
    assert!(ZipReader::open(Cursor::new(&bytes)).unwrap().find("resources/assets/smart-materials/worn.mcfrag").is_some());
    doc.assets.push(EmbeddedAsset::new("materials/rust.mca", vec![]));
    assert!(crate::paint::format::save(&doc, &mut Vec::new()).is_err(), "an asset embedded twice");
}
