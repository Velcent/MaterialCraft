//! Saving: what is written reads back as the document, the assets it uses are embedded first, and the dirty marker
//! follows the history.

use super::*;
use materialcraft::paint::edit::LayerSlot;
use materialcraft::paint::edit::ops::{DropPayload, DropTarget, MaterialAsset, drop_onto};
use materialcraft::paint::{BlendMode, Channel, EmbeddedAsset, Layer, LayerKind, PaintContent, TextureSet};

fn open(source: &str) -> OpenDocument {
    let mut set = TextureSet::new("s", "S", (16, 16)).with_channel(Channel::BaseColor);
    set.layers.push(Layer::new("base", "Base", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    // Embedded earlier, used by nothing any more.
    doc.assets.push(EmbeddedAsset::new("materials/old.mca", b"old".to_vec()));
    OpenDocument::new(doc, PathBuf::from(source), None)
}

/// One history step: a material fill layer on top (it uses `materials/brick.mca`).
fn add_material(d: &mut OpenDocument) {
    let payload = DropPayload::Material(MaterialAsset { name: "Brick".into(), path: "materials/brick.mca".into(), params: Vec::new() });
    let target = DropTarget::Between { set: "s".into(), at: LayerSlot::top(1) };
    d.edit("Drop", None, |tx| drop_onto(tx, &target, &payload, false)).unwrap();
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mc-paint-save-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_save_embeds_what_the_layers_use_and_reads_back_as_the_document() {
    let dir = temp_dir("round-trip");
    let mut d = open("robot.spp");
    assert_eq!(d.file(), None, "an import has no file: Save asks where");
    add_material(&mut d);
    assert!(d.is_dirty());

    let mut asked = Vec::new();
    let (copy, report) = prepare(&mut d, |r| {
        asked.push(r.to_string());
        (r == "materials/brick.mca").then(|| b"brick graph".to_vec())
    });
    assert_eq!(asked, ["materials/brick.mca"], "embed_used looked up the material the new layer uses");
    assert_eq!((report.added, report.removed), (vec!["materials/brick.mca".to_string()], vec!["materials/old.mca".to_string()]));
    assert_eq!(d.doc().assets.iter().map(|a| a.reference.as_str()).collect::<Vec<_>>(), ["materials/brick.mca"]);
    assert_eq!(&copy, d.doc());

    let path = with_extension(dir.join("robot"));
    SaveJob::start(copy, path.clone(), None).wait().unwrap();
    d.saved_to(path.clone());
    assert!(!d.is_dirty());
    assert_eq!((d.file(), d.source()), (Some(path.as_path()), path.as_path()), "Save writes there from now on");
    let back = format::load_path(&path).unwrap();
    assert_eq!(&back, d.doc(), "save → load round trip");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_dirty_marker_follows_undo_and_redo() {
    let mut d = open("robot.mcpaint");
    assert!(!d.is_dirty(), "just opened");
    assert_eq!(d.title(), "robot.mcpaint");
    add_material(&mut d);
    assert!(d.is_dirty());
    assert!(d.title().starts_with("robot.mcpaint ") && d.title() != "robot.mcpaint");
    d.undo().unwrap();
    assert!(!d.is_dirty(), "undone back to the file's state");
    d.redo().unwrap();
    assert!(d.is_dirty());

    // Saved after the edit: undo makes it dirty, redo clean again.
    let _ = d.begin_save();
    d.saved_to(PathBuf::from("robot.mcpaint"));
    assert!(!d.is_dirty());
    d.undo().unwrap();
    assert!(d.is_dirty());
    d.redo().unwrap();
    assert!(!d.is_dirty());

    // An edit made while the save runs is not in the file.
    let _ = d.begin_save();
    add_material(&mut d);
    d.saved_to(PathBuf::from("robot.mcpaint"));
    assert!(d.is_dirty());
    d.undo().unwrap();
    assert!(!d.is_dirty(), "the saved state is the one the save started from");

    // A failed save changes nothing.
    d.redo().unwrap();
    let _ = d.begin_save();
    d.save_failed();
    assert!(d.is_dirty());
}

#[test]
fn a_failed_save_reports_and_leaves_no_file() {
    let dir = temp_dir("fail");
    let path = dir.join("missing-folder").join("doc.mcpaint");
    let err = SaveJob::start(Document::default(), path.clone(), None).wait().unwrap_err();
    assert!(!err.is_empty());
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
