//! Roadmap step 17 gate: a document that embeds the library assets it uses (AS-21) opens and composites the same
//! after the library folder is gone; a smart material saved into a library and dropped back is one undo step with
//! ids unique in the texture set.

use materialcraft::material_fill::{ChainResolver, FolderResolver, MaterialCache, MaterialFills, MaterialResolver, MemoryResolver};
use materialcraft::paint::edit::ops::{DropOutcome, DropPayload, DropTarget, drop_onto};
use materialcraft::paint::edit::{LayerSlot, Transaction};
use materialcraft::paint::{
    BlendMode, Channel, Compositor, Document, Fill, FillContext, FillSource, Fragment, FragmentContent, Layer, LayerKind, TextureSet, embedded, format,
};
use std::path::Path;
use std::sync::Arc;

const GRAPH: &str = "size 16\nnode 1 color r=0.2 g=0.4 b=0.6\nnode 2 gradient mode=0 angle=0\noutput basecolor 1\noutput roughness 2\n";

fn document() -> Document {
    let mut set = TextureSet::new("s", "S", (16, 16)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    let fill = Fill::new(FillSource::Material { path: "materials/blue.mca".into(), params: vec![] });
    let layer = Layer::new("m", "Blue", LayerKind::Fill(fill)).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0).with_blend(
        Channel::Roughness,
        BlendMode::Normal,
        1.0,
    );
    set.layers.push(layer);
    Document { texture_sets: vec![set], ..Document::default() }
}

/// The base colour and roughness of `doc`'s first set with materials looked up through `resolver`.
fn composite(doc: &Document, resolver: impl MaterialResolver + 'static) -> Vec<Vec<[f32; 4]>> {
    let fills = MaterialFills::new(Arc::new(resolver), Arc::new(MaterialCache::default()));
    let context = FillContext::with_sources(&fills);
    let set = &doc.texture_sets[0];
    [Channel::BaseColor, Channel::Roughness].into_iter().map(|c| Compositor::new(set).with_fills(&context).composite_tex(c).0.px).collect()
}

/// What the editor does when it opens a document: embedded assets first, then the library folders.
fn opened(doc: &Document, library: &Path) -> ChainResolver {
    ChainResolver::new([
        Arc::new(MemoryResolver::from_embedded(&doc.assets)) as Arc<dyn MaterialResolver>,
        Arc::new(FolderResolver::new([library.to_path_buf()])),
    ])
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("materialcraft-embedded-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("materials")).unwrap();
    dir
}

#[test]
fn a_document_with_embedded_assets_opens_without_the_library() {
    let library = scratch("library");
    std::fs::write(library.join("materials/blue.mca"), GRAPH).unwrap();
    let mut doc = document();
    let folders = FolderResolver::new([library.clone()]);
    let with_library = composite(&doc, folders.clone());

    // Saving embeds what the document uses.
    let report = embedded::embed_used(&mut doc, |r| std::fs::read(folders.locate(r)?).ok());
    assert_eq!(report.added, ["materials/blue.mca"]);
    let file = library.with_extension("mcpaint");
    format::save_path(&doc, &file).unwrap();

    std::fs::remove_dir_all(&library).unwrap();
    let back = format::load_path(&file).unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(back, doc);
    assert_eq!(composite(&back, opened(&back, &library)), with_library, "same pixels without the library");
    // Without the embedded copy the material would be missing.
    assert_ne!(composite(&document(), FolderResolver::new([library.clone()])), with_library);
}

#[test]
fn a_saved_smart_material_drops_back_as_one_step_with_unique_ids() {
    let library = scratch("smart");
    let mut doc = document();
    let set = &doc.texture_sets[0];
    let fragment = Fragment::smart_material("Blue paint", set, set.layers.clone());
    let path = library.join("blue-paint.mcfrag");
    format::fragment::save_path(&fragment, &path).unwrap();
    let loaded = format::fragment::load_path(&path).unwrap();
    let _ = std::fs::remove_dir_all(&library);
    assert_eq!(loaded, fragment);

    let FragmentContent::Layers(layers) = loaded.content else { panic!("a smart material holds layers") };
    let payload = DropPayload::Folder { name: loaded.name, layers, asset: Some("smart-materials/blue-paint.mcfrag".into()) };
    let before = doc.clone();
    let mut tx = Transaction::new(&mut doc);
    let out = drop_onto(&mut tx, &DropTarget::Between { set: "s".into(), at: LayerSlot::top(1) }, &payload, false).unwrap();
    let undo = tx.commit();
    doc.validate().unwrap();
    let DropOutcome::NewLayer(id) = out else { panic!("{out:?}") };
    let folder = doc.texture_sets[0].layer(&id).unwrap();
    assert_eq!(folder.asset.as_deref(), Some("smart-materials/blue-paint.mcfrag"));
    assert_ne!(folder.children()[0].id, "m", "ids are unique in the set");
    assert_eq!(undo.len(), 1, "one undo step");
    for e in undo.into_iter().rev() {
        e.apply(&mut doc).unwrap();
    }
    assert_eq!(doc, before);
}
