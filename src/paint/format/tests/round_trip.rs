//! save → load gives the same document, and saving is deterministic.

use super::fixtures::{full_document, noisy};
use crate::paint::format::{self, SaveOptions, Thumbnail, load, save, save_with};
use crate::paint::{Channel, Document, LayerKind, PixelFormat, TextureSet};
use std::io::Cursor;

pub fn to_bytes(doc: &Document) -> Vec<u8> {
    let mut out = Vec::new();
    save(doc, &mut out).unwrap();
    out
}

pub fn from_bytes(bytes: &[u8]) -> Result<Document, format::FormatError> {
    load(Cursor::new(bytes))
}

#[test]
fn full_document_round_trips() {
    let doc = full_document();
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back.mesh, doc.mesh);
    assert_eq!(back.foreign, doc.foreign);
    assert_eq!(back.import_report, doc.import_report);
    for (a, b) in back.texture_sets.iter().zip(&doc.texture_sets) {
        for ((_, la), (_, lb)) in a.walk_layers().zip(b.walk_layers()) {
            assert_eq!(la, lb, "layer {}", lb.id);
        }
        assert_eq!(a, b);
    }
    assert_eq!(back, doc);
}

#[test]
fn unallocated_tiles_stay_unallocated() {
    let doc = full_document();
    let back = from_bytes(&to_bytes(&doc)).unwrap();
    let count =
        |d: &Document| -> usize { d.texture_sets.iter().flat_map(|s| s.walk_layers()).flat_map(|(_, l)| l.images()).map(|(_, i)| i.allocated_tiles()).sum() };
    assert!(count(&back) <= count(&doc));
}

#[test]
fn saving_is_deterministic() {
    let doc = full_document();
    let a = to_bytes(&doc);
    assert_eq!(a, to_bytes(&doc));
    assert_eq!(a, to_bytes(&from_bytes(&a).unwrap()), "load → save reproduces the file");
}

#[test]
fn small_documents_round_trip() {
    let empty = Document::default();
    assert_eq!(from_bytes(&to_bytes(&empty)).unwrap(), empty);

    let mut doc = Document::default();
    doc.texture_sets.push(TextureSet::new("a", "A", (1, 1)));
    let mut big_float = TextureSet::new("b", "B", (300, 5)).with_channel(Channel::Height);
    let img = noisy(300, 5, PixelFormat::from_id("graya16f").unwrap(), 4);
    big_float.layers.push(crate::paint::Layer::new("h", "H", LayerKind::Paint(crate::paint::PaintContent::from_images(vec![(Channel::Height, img)]))));
    doc.texture_sets.push(big_float);
    for thumbnail in [Thumbnail::None, Thumbnail::FromDocument] {
        let mut out = Vec::new();
        save_with(&doc, &mut out, &SaveOptions { thumbnail }).unwrap();
        assert_eq!(from_bytes(&out).unwrap(), doc);
    }
}

#[test]
fn invalid_documents_are_not_saved() {
    let mut doc = full_document();
    doc.texture_sets[0].resolution = (10, 10);
    assert!(matches!(save(&doc, Vec::new()), Err(format::FormatError::Invalid(_))));
    let mut doc = full_document();
    doc.foreign[0].path = "substance/document.bin".into();
    assert!(matches!(save(&doc, Vec::new()), Err(format::FormatError::Unsupported(_))));
    let mut doc = full_document();
    doc.mesh.parts[0].triangles.push([0, 1, 99]);
    assert!(matches!(save(&doc, Vec::new()), Err(format::FormatError::InvalidMesh(_))));
}

#[test]
fn save_path_replaces_atomically() {
    let dir = std::env::temp_dir().join(format!("mcpaint-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("doc.mcpaint");
    let doc = full_document();
    format::save_path(&doc, &path).unwrap();
    format::save_path(&doc, &path).unwrap();
    assert_eq!(format::load_path(&path).unwrap(), doc);
    let mut bad = doc.clone();
    bad.texture_sets[0].resolution = (1, 1);
    assert!(format::save_path(&bad, &path).is_err());
    assert_eq!(format::load_path(&path).unwrap(), doc, "a failed save keeps the old file");
    let names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["doc.mcpaint"], "no temporary files left");
    std::fs::remove_dir_all(&dir).unwrap();
}
