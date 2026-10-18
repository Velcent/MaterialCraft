//! A document embeds copies of the library assets it uses (`paint::embedded`), so it still opens, and its material
//! still resolves, after the asset was renamed or deleted in the library. Documents keep referring to assets by the
//! reference they were made with; the store doesn't touch them.

use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::assets::AssetStore;
use crate::material_fill::{ChainResolver, FolderResolver, MaterialResolver, MemoryResolver};
use crate::paint::embedded::embed_used;
use crate::paint::format::{load, save};
use crate::paint::{Channel, Document, Fill, FillSource, Layer, LayerKind, TextureSet};

const GRAPH: &[u8] = b"{\"graph\": \"rust\"}";

/// A document whose fill uses `materials/rust.mca`, with that asset embedded from `library`, saved.
fn saved_document(library: &Path) -> Vec<u8> {
    let mut set = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::BaseColor);
    let source = FillSource::Material { path: "materials/rust.mca".into(), params: vec![] };
    set.layers.push(Layer::new("rust", "Rust", LayerKind::Fill(Fill::new(source))));
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    let report = embed_used(&mut doc, |r| fs::read(library.join(r)).ok());
    assert_eq!(report.added, ["materials/rust.mca"]);
    let mut bytes = Vec::new();
    save(&doc, &mut bytes).unwrap();
    bytes
}

/// Opens the saved document, resolves its material the way the editor does (embedded copies first, then the
/// library) and saves it again (which re-embeds what it uses).
fn reopen(bytes: &[u8], library: &Path) {
    let mut doc = load(Cursor::new(bytes)).unwrap();
    assert_eq!(doc.assets.len(), 1);
    let resolver = ChainResolver::new([
        Arc::new(MemoryResolver::from_embedded(&doc.assets)) as Arc<dyn MaterialResolver>,
        Arc::new(FolderResolver::new([library.to_path_buf()])),
    ]);
    assert_eq!(resolver.resolve("materials/rust.mca").unwrap().as_bytes(), GRAPH);
    let report = embed_used(&mut doc, |r| fs::read(library.join(r)).ok());
    assert!(report.missing.is_empty() && !report.changed(), "the embedded copy stays: {report:?}");
    save(&doc, &mut Vec::new()).unwrap();
}

#[test]
fn a_document_opens_after_its_asset_was_renamed_or_deleted() {
    let dir = temp_dir("documents");
    let store = AssetStore::new();
    let root = user_root(&dir);
    store.create(&root, "materials/rust.mca", &Draft::new(GRAPH)).unwrap();
    let bytes = saved_document(&dir);

    store.rename(&root, "materials/rust.mca", "Old rust").unwrap();
    assert!(!dir.join("materials/rust.mca").exists());
    reopen(&bytes, &dir);

    store.delete(&root, "materials/Old rust.mca").unwrap();
    assert!(snapshot(&dir.join("materials")).is_empty());
    reopen(&bytes, &dir);
    fs::remove_dir_all(&dir).unwrap();
}
