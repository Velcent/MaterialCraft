//! Each operation does what it says, and an asset's three files always go together.

use std::fs;

use super::*;
use crate::assets::{AssetRef, AssetStore, MetaEdit, RootId};
use crate::json;

#[test]
fn create_writes_the_file_its_sidecar_and_its_thumbnail() {
    let dir = temp_dir("create");
    let store = AssetStore::new();
    let side = sidecar("Scratch");
    let change = store.create(&user_root(&dir), "alphas/deep/scratch.png", &full(&side)).unwrap();
    assert_eq!(change.before, None);
    assert_eq!(change.after, Some(AssetRef::new(RootId::new("user"), "alphas/deep/scratch.png")));
    let s = snapshot(&dir);
    assert_eq!(names(&s), ["alphas/", "alphas/deep/", "alphas/deep/scratch.png", "alphas/deep/scratch.png.mcmeta", "alphas/deep/scratch.png.thumb.png"]);
    assert_eq!(s["alphas/deep/scratch.png"], b"asset bytes");
    let text = String::from_utf8(s["alphas/deep/scratch.png.mcmeta"].clone()).unwrap();
    assert_eq!(text, format!("{}\n", side.to_json().to_string_pretty()), "the sidecar's text is what writers wrote before the store");
    assert_eq!(Sidecar::parse(&text).unwrap(), side);

    // Without companions only the file is written; a named asset is numbered while any of its files is taken.
    store.create(&user_root(&dir), "bare.png", &Draft::new(b"x")).unwrap();
    assert!(dir.join("bare.png").is_file() && !dir.join("bare.png.mcmeta").exists());
    fs::create_dir_all(dir.join("alphas")).unwrap();
    fs::write(dir.join("alphas/Rust_ worn.png.mcmeta"), "{}").unwrap();
    let named = store.create_named(&user_root(&dir), Usage::Alpha, "Rust: worn", "png", &Draft::new(b"y")).unwrap();
    assert_eq!(named.after.unwrap().path, "alphas/Rust_ worn 2.png", "an orphan sidecar holds the name");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn update_changes_name_tags_and_description_and_keeps_everything_else() {
    let dir = temp_dir("update");
    let store = AssetStore::new();
    let root = user_root(&dir);
    store.create(&root, "alphas/a.png", &full(&sidecar("A"))).unwrap();
    // A field a newer MaterialCraft wrote survives the edit.
    let path = dir.join("alphas/a.png.mcmeta");
    let mut v = json::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    v.insert("future", "kept");
    fs::write(&path, v.to_string_pretty()).unwrap();

    let edit = MetaEdit { name: Some(" Better ".into()), tags: Some(MetaEdit::tags_from_text("worn, metal,, worn ")), description: Some(None) };
    let change = store.update(&root, "alphas/a.png", &edit).unwrap();
    assert_eq!(change.before, change.after);
    let v = json::parse(&fs::read_to_string(&path).unwrap()).unwrap();
    let back = Sidecar::parse(&v.to_string_pretty()).unwrap();
    assert_eq!(back.name.as_deref(), Some("Better"));
    assert_eq!(back.tags, ["worn", "metal"]);
    assert_eq!(back.description, None);
    assert_eq!(back.provenance, Provenance::HandMade { author: None, notes: None }, "untouched fields stay");
    assert_eq!(v.get("future").and_then(json::Value::as_str), Some("kept"));

    // An asset without a sidecar gets one.
    store.create(&root, "textures/t.png", &Draft::new(b"t")).unwrap();
    store.update(&root, "textures/t.png", &MetaEdit { tags: Some(vec!["new".into()]), ..MetaEdit::default() }).unwrap();
    let made = Sidecar::parse(&fs::read_to_string(dir.join("textures/t.png.mcmeta")).unwrap()).unwrap();
    assert_eq!((made.tags, made.name), (vec!["new".to_string()], None));
    assert!(snapshot(&dir).keys().all(|k| !k.contains("/.")), "no temporary files are left");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn rename_renames_all_three_files_and_names_the_asset() {
    let dir = temp_dir("rename");
    let store = AssetStore::new();
    let root = user_root(&dir);
    store.create(&root, "alphas/a.png", &full(&sidecar("A"))).unwrap();
    let change = store.rename(&root, "alphas/a.png", "Edge wear: soft").unwrap();
    assert_eq!(change.after.as_ref().unwrap().path, "alphas/Edge wear_ soft.png");
    let s = snapshot(&dir);
    assert_eq!(names(&s), ["alphas/", "alphas/Edge wear_ soft.png", "alphas/Edge wear_ soft.png.mcmeta", "alphas/Edge wear_ soft.png.thumb.png"]);
    assert_eq!(s["alphas/Edge wear_ soft.png"], b"asset bytes");
    assert_eq!(s["alphas/Edge wear_ soft.png.thumb.png"], b"thumbnail bytes");
    let side = Sidecar::parse(std::str::from_utf8(&s["alphas/Edge wear_ soft.png.mcmeta"]).unwrap()).unwrap();
    assert_eq!((side.name.as_deref(), side.tags), (Some("Edge wear: soft"), vec!["old".to_string()]));

    // A name giving the same file name only changes the sidecar.
    let same = store.rename(&root, "alphas/Edge wear_ soft.png", "Edge wear? soft").unwrap();
    assert_eq!(same.before, same.after);
    let side = Sidecar::parse(&fs::read_to_string(dir.join("alphas/Edge wear_ soft.png.mcmeta")).unwrap()).unwrap();
    assert_eq!(side.name.as_deref(), Some("Edge wear? soft"));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn move_within_a_library_and_into_another_carries_all_three_files() {
    let (a, b) = (temp_dir("move-a"), temp_dir("move-b"));
    let store = AssetStore::new();
    let (user, extra) = (user_root(&a), extra_root(&b));
    let mut side = sidecar("A");
    side.thumbnail = Some("../pictures/a.png".into());
    store.create(&user, "alphas/a.png", &full(&side)).unwrap();

    // Within the library: renamed into the new folder; the sidecar's own picture path now starts from there.
    let change = store.move_to(&user, "alphas/a.png", &user, "alphas/worn").unwrap();
    assert_eq!(change.after, Some(AssetRef::new(RootId::new("user"), "alphas/worn/a.png")));
    let s = snapshot(&a);
    assert_eq!(names(&s), ["alphas/", "alphas/worn/", "alphas/worn/a.png", "alphas/worn/a.png.mcmeta", "alphas/worn/a.png.thumb.png"]);
    let moved = Sidecar::parse(std::str::from_utf8(&s["alphas/worn/a.png.mcmeta"]).unwrap()).unwrap();
    assert_eq!(moved.thumbnail.as_deref(), Some("../../pictures/a.png"));
    assert_eq!(Sidecar { thumbnail: None, ..moved }, Sidecar { thumbnail: None, ..side.clone() });

    // Into another library: copied there, the originals gone; a picture left behind is no longer named.
    let change = store.move_to(&user, "alphas/worn/a.png", &extra, "").unwrap();
    assert_eq!(change.after, Some(AssetRef::new(RootId::new("extra"), "a.png")));
    assert_eq!(change.warnings.len(), 1, "{:?}", change.warnings);
    assert_eq!(names(&snapshot(&a)), ["alphas/", "alphas/worn/"]);
    let s = snapshot(&b);
    assert_eq!(names(&s), ["a.png", "a.png.mcmeta", "a.png.thumb.png"]);
    assert_eq!((s["a.png"].as_slice(), s["a.png.thumb.png"].as_slice()), (&b"asset bytes"[..], &b"thumbnail bytes"[..]));
    assert_eq!(Sidecar::parse(std::str::from_utf8(&s["a.png.mcmeta"]).unwrap()).unwrap().thumbnail, None);

    // Back again, without a sidecar: just the file and the thumbnail.
    fs::remove_file(b.join("a.png.mcmeta")).unwrap();
    store.move_to(&extra, "a.png", &user, "alphas").unwrap();
    assert_eq!(names(&snapshot(&a)), ["alphas/", "alphas/a.png", "alphas/a.png.thumb.png", "alphas/worn/"]);
    assert!(snapshot(&b).is_empty());
    fs::remove_dir_all(&a).unwrap();
    fs::remove_dir_all(&b).unwrap();
}

#[test]
fn delete_removes_all_three_files() {
    let dir = temp_dir("delete");
    let store = AssetStore::new();
    let root = user_root(&dir);
    store.create(&root, "alphas/a.png", &full(&sidecar("A"))).unwrap();
    store.create(&root, "alphas/b.png", &Draft::new(b"b")).unwrap();
    let change = store.delete(&root, "alphas/a.png").unwrap();
    assert_eq!((change.before.unwrap().path.as_str(), change.after), ("alphas/a.png", None));
    assert_eq!(names(&snapshot(&dir)), ["alphas/", "alphas/b.png"]);
    store.delete(&root, "alphas/b.png").unwrap();
    assert_eq!(names(&snapshot(&dir)), ["alphas/"], "the folder stays");
    fs::remove_dir_all(&dir).unwrap();
}
