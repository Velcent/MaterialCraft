//! Headless gate for the asset library core (roadmap step 9): a synthetic library of 10k files,
//! query speed, watching by polling, malformed sidecars, and the search syntax (ADR 0008).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use materialcraft::assets::{
    Asset, ChangeKind, Field, Format, Library, LibraryRoot, Listing, Location, Mode, Provenance, Query, RootId, RootKind, SavedSearch, Search, Sidecar,
    Thumbnail, Usage, create_standard_layout,
};
use materialcraft::diag::{Area, Entry, Level, Sink};

/// A sink that keeps everything, for assertions.
#[derive(Default)]
struct Keep(Mutex<Vec<Entry>>);

impl Sink for Keep {
    fn emit(&self, entry: Entry) {
        self.0.lock().unwrap().push(entry);
    }
}

impl Keep {
    fn take(&self) -> Vec<Entry> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// A fresh, empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("assets-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, rel: &str, text: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

const WORDS: [&str; 12] = ["rust", "steel", "brick", "moss", "leather", "scratch", "dirt", "paint", "wood", "stone", "fabric", "edge"];

/// 10 000 files: 8 000 assets spread over the usage folders and subfolders, 1 900 sidecars, 100
/// thumbnails. Returns the folder.
fn synthetic_library(name: &str) -> PathBuf {
    let dir = scratch(name);
    let folders = Usage::ALL.map(|u| u.folder());
    for i in 0..8000usize {
        let usage_folder = folders[i % folders.len()];
        let sub = WORDS[(i / 9) % WORDS.len()];
        let word = WORDS[i % WORDS.len()];
        let ext = ["mca", "png", "exr", "mca", "png", "sbsar"][i % 6];
        let rel = format!("{usage_folder}/{sub}/{word}-{i:05}.{ext}");
        write(&dir, &rel, "x");
        if i < 1900 {
            let tag = WORDS[(i + 5) % WORDS.len()];
            write(
                &dir,
                &format!("{rel}.mcmeta"),
                &format!(r#"{{"name": "{word} {i}", "tags": ["{tag}", "set{}"], "provenance": {{"kind": "generator", "generator": "g{}"}}}}"#, i % 7, i % 3),
            );
        } else if i < 2000 {
            write(&dir, &format!("{rel}.thumb.png"), "t");
        }
    }
    dir
}

#[test]
fn a_ten_thousand_file_library_scans_and_answers_queries_in_under_10_ms() {
    let dir = synthetic_library("big");
    let sink = Keep::default();
    let started = Instant::now();
    let lib = Library::with_roots([LibraryRoot::folder("user", "My assets", RootKind::User, &dir)], &sink);
    let scan = started.elapsed();
    let cat = lib.catalogue();
    assert_eq!(cat.len(), 8000);
    assert!(sink.take().is_empty(), "a clean library reports nothing");

    let queries: [(&str, Search); 7] = [
        ("contains", Search::text("rust")),
        ("exact tag", Search::text("tag:!moss")),
        ("phrase + exclusion", Search::text(r#""steel 1" -format:sbsar"#)),
        ("several terms", Search::text("usage:material, path:brick -tag:set3 source:generator")),
        ("type filter", Search::text("e").with_usages([Usage::Alpha, Usage::Brush])),
        ("folder filter", Search::text("-tag:moss 0").with_location(Location::folder(RootId::new("user"), "textures/wood"))),
        ("everything", Search::all()),
    ];
    for (label, search) in &queries {
        // Best of five, so a busy machine (other agents run builds) doesn't fail the gate.
        let mut best = Duration::MAX;
        let mut n = 0;
        for _ in 0..5 {
            let t = Instant::now();
            n = cat.find(search).len();
            best = best.min(t.elapsed());
        }
        println!("query {label:<20} {n:>5} hits in {best:?}");
        assert!(best < Duration::from_millis(10), "{label} took {best:?}");
    }
    println!("scan of 10k files: {scan:?}");

    // Spot checks on the results.
    assert_eq!(cat.find(&Search::all()).len(), 8000);
    assert!(cat.find(&Search::text("tag:!moss")).iter().all(|h| h.asset.tags.iter().any(|t| t == "moss")));
    assert!(cat.find(&Search::text("-format:sbsar")).iter().all(|h| h.asset.format != Format::Sbsar));
    let in_folder = cat.find(&Search::all().with_location(Location::folder(RootId::new("user"), "textures/wood")));
    assert!(!in_folder.is_empty() && in_folder.iter().all(|h| h.asset.path.starts_with("textures/wood/")));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_watcher_picks_up_new_changed_and_removed_files_within_one_poll() {
    let dir = synthetic_library("watch");
    let sink = Keep::default();
    let mut lib = Library::with_roots([LibraryRoot::folder("user", "User", RootKind::User, &dir)], &sink);
    assert!(lib.poll(&sink).is_empty(), "nothing changed yet");

    // New file.
    write(&dir, "materials/new/copper.mca", "graph");
    let t = Instant::now();
    let changes = lib.poll(&sink).assets;
    println!("poll over 10k files: {:?}", t.elapsed());
    assert_eq!(changes.len(), 1);
    assert_eq!((changes[0].path.as_str(), changes[0].kind), ("materials/new/copper.mca", ChangeKind::Added));
    assert_eq!(lib.catalogue().find(&Search::text("name:!copper")).len(), 1);
    assert!(lib.catalogue().folders(&RootId::new("user")).any(|f| f == "materials/new"));

    // Changed file: same length, only the time moves (the watcher compares time and size).
    let p = dir.join("materials/new/copper.mca");
    fs::write(&p, "GRAPH").unwrap();
    fs::File::options().write(true).open(&p).unwrap().set_modified(SystemTime::now() + Duration::from_secs(5)).unwrap();
    let changes = lib.poll(&sink).assets;
    assert_eq!(changes.iter().map(|c| c.kind).collect::<Vec<_>>(), [ChangeKind::Modified]);

    // A new sidecar changes the asset it belongs to.
    write(&dir, "materials/new/copper.mca.mcmeta", r#"{"name": "Polished copper", "tags": ["metal"]}"#);
    let changes = lib.poll(&sink).assets;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].kind, ChangeKind::Modified);
    assert_eq!(lib.catalogue().find(&Search::text(r#"name:!"polished copper" tag:metal"#)).len(), 1);

    // A new thumbnail too.
    write(&dir, "materials/new/copper.mca.thumb.png", "png");
    assert_eq!(lib.poll(&sink).assets.len(), 1);
    let a = lib.catalogue().get(&RootId::new("user"), "materials/new/copper.mca").unwrap();
    assert_eq!(a.thumbnail, Thumbnail::File("materials/new/copper.mca.thumb.png".into()));

    // Removed file; its leftover sidecar is reported.
    fs::remove_file(&p).unwrap();
    let changes = lib.poll(&sink).assets;
    assert_eq!(changes.iter().map(|c| (c.path.as_str(), c.kind)).collect::<Vec<_>>(), [("materials/new/copper.mca", ChangeKind::Removed)]);
    assert!(lib.catalogue().get(&RootId::new("user"), "materials/new/copper.mca").is_none());
    assert_eq!(lib.catalogue().len(), 8000);
    assert!(sink.take().iter().all(|e| e.level == Level::Warning && e.text.contains("sidecar without an asset")));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_sidecars_are_reported_and_never_fatal() {
    let dir = scratch("malformed");
    let bad = [
        ("a.mca", "{not json"),
        ("b.mca", "[1, 2]"),
        ("c.mca", r#"{"usage": "spaceship"}"#),
        ("d.mca", r#"{"tags": "metal"}"#),
        ("e.mca", r#"{"version": 99}"#),
        ("f.mca", r#"{"format": "something-else"}"#),
        ("g.mca", r#"{"provenance": {"kind": "cc0"}}"#),
        ("h.mca", r#"{"provenance": {"kind": "cc0", "url": "https://example.org/x", "licence": "CC-BY-4.0"}}"#),
        ("i.mca", "\u{0}\u{1}binary"),
        ("j.mca", ""),
    ];
    for (asset, sidecar) in bad {
        write(&dir, &format!("materials/{asset}"), "g");
        write(&dir, &format!("materials/{asset}.mcmeta"), sidecar);
    }
    fs::write(dir.join("materials/k.mca"), "g").unwrap();
    fs::write(dir.join("materials/k.mca.mcmeta"), [0xff, 0xfe, 0x00]).unwrap(); // not UTF-8
    write(&dir, "materials/ok.mca", "g");
    write(&dir, "materials/ok.mca.mcmeta", r#"{"name": "Fine", "usage": "smart material"}"#);
    write(&dir, "materials/orphan.mca.mcmeta", "{}");

    let sink = Keep::default();
    let lib = Library::with_roots([LibraryRoot::folder("u", "U", RootKind::User, &dir)], &sink);
    let entries = sink.take();
    let errors: Vec<_> = entries.iter().filter(|e| e.level == Level::Error).collect();
    assert_eq!(errors.len(), 11, "one error per bad sidecar: {entries:#?}");
    assert!(entries.iter().all(|e| e.area == Area::Assets && e.target.is_some()));
    assert_eq!(entries.iter().filter(|e| e.text.contains("sidecar without an asset")).count(), 1);

    // Every asset is still there, with its folder-derived metadata.
    let cat = lib.catalogue();
    assert_eq!(cat.len(), 12);
    let a = cat.get(&RootId::new("u"), "materials/a.mca").unwrap();
    assert_eq!((a.name.as_str(), a.usage, a.has_sidecar), ("a", Usage::Material, false));
    let ok = cat.get(&RootId::new("u"), "materials/ok.mca").unwrap();
    assert_eq!((ok.name.as_str(), ok.usage, ok.has_sidecar), ("Fine", Usage::SmartMaterial, true));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn usage_comes_from_the_sidecar_then_the_folder_then_the_format() {
    let dir = scratch("usage");
    write(&dir, "Smart Masks/edges/wear.mca", "g");
    write(&dir, "misc/scratches.png", "p");
    write(&dir, "misc/room.exr", "e");
    write(&dir, "misc/graph.mca", "g");
    write(&dir, "brushes/pencil.png", "p");
    write(&dir, "textures/env.exr", "e");
    write(&dir, "textures/env.exr.mcmeta", r#"{"usage": "environment"}"#);
    write(&dir, "materials/old.sbsar", "s");
    write(&dir, "materials/notes.txt", "ignored");
    write(&dir, ".hidden/x.mca", "ignored");
    let lib = Library::with_roots([LibraryRoot::folder("u", "U", RootKind::User, &dir)], &Keep::default());
    let cat = lib.catalogue();
    let usage = |p: &str| cat.get(&RootId::new("u"), p).map(|a| a.usage);
    assert_eq!(usage("Smart Masks/edges/wear.mca"), Some(Usage::SmartMask));
    assert_eq!(usage("misc/scratches.png"), Some(Usage::Texture));
    assert_eq!(usage("misc/room.exr"), Some(Usage::Texture));
    assert_eq!(usage("misc/graph.mca"), Some(Usage::Material));
    assert_eq!(usage("brushes/pencil.png"), Some(Usage::Brush));
    assert_eq!(usage("textures/env.exr"), Some(Usage::Environment));
    assert_eq!(cat.len(), 7);

    // .sbsar is listed, marked importable only, and can be hidden.
    let sbsar = cat.get(&RootId::new("u"), "materials/old.sbsar").unwrap();
    assert!(sbsar.is_importable_only());
    let mut s = Search::all();
    s.hide_importable = true;
    assert_eq!(cat.find(&s).len(), 6);
    assert_eq!(cat.find(&Search::text("format:sbsar")).len(), 1);
    // Images are their own thumbnail.
    assert_eq!(cat.get(&RootId::new("u"), "misc/scratches.png").unwrap().thumbnail, Thumbnail::OwnImage);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn sidecars_round_trip_with_provenance() {
    let cases = [
        Provenance::Unknown,
        Provenance::Graph { graph: "graphs/steel.mca".into(), parameters: Some(materialcraft::json::parse(r#"{"seed": 4}"#).unwrap()) },
        Provenance::Generator { generator: "edge-wear".into(), preset: None },
        Provenance::HandMade { author: Some("Felix".into()), notes: None },
        Provenance::Cc0 { url: "https://example.org/rock".into(), licence: "CC0-1.0".into(), author: None },
    ];
    for provenance in cases {
        let s = Sidecar {
            name: Some("Steel".into()),
            usage: Some(Usage::SmartMaterial),
            tags: vec!["metal".into(), "clean".into()],
            description: Some("d".into()),
            thumbnail: Some("../thumbs/steel.png".into()),
            needs: vec!["curvature".into()],
            provenance,
        };
        let text = s.to_json().to_string_pretty();
        assert_eq!(Sidecar::parse(&text).unwrap(), s, "{text}");
    }
    // Unknown fields are ignored; a missing format marker is fine.
    assert_eq!(Sidecar::parse(r#"{"future": 1}"#).unwrap(), Sidecar::default());
}

#[test]
fn sidecar_thumbnails_resolve_relative_to_the_asset_and_stay_inside_the_library() {
    let dir = scratch("thumbs");
    write(&dir, "materials/metal/steel.mca", "g");
    write(&dir, "materials/metal/steel.mca.mcmeta", r#"{"thumbnail": "../../thumbs/steel.png"}"#);
    write(&dir, "materials/metal/iron.mca", "g");
    write(&dir, "materials/metal/iron.mca.mcmeta", r#"{"thumbnail": "../../../outside.png"}"#);
    let sink = Keep::default();
    let lib = Library::with_roots([LibraryRoot::folder("u", "U", RootKind::User, &dir)], &sink);
    let get = |p: &str| lib.catalogue().get(&RootId::new("u"), p).unwrap().thumbnail.clone();
    assert_eq!(get("materials/metal/steel.mca"), Thumbnail::File("thumbs/steel.png".into()));
    assert_eq!(get("materials/metal/iron.mca"), Thumbnail::None);
    assert_eq!(sink.take().len(), 1);
    fs::remove_dir_all(dir).unwrap();
}

fn terms(text: &str) -> Vec<(Field, Mode, String, bool)> {
    Query::parse(text).terms().iter().map(|t| (t.field, t.mode, t.value.clone(), t.exclude)).collect()
}

#[test]
fn the_search_syntax_follows_the_settled_rules() {
    use Field::*;
    use Mode::*;
    let s = |v: &str| v.to_string();
    assert_eq!(terms("Rust"), [(Any, Contains, s("rust"), false)]);
    assert_eq!(terms("!rust"), [(Any, Exact, s("rust"), false)]);
    assert_eq!(terms(r#""old rust" steel"#), [(Any, Contains, s("old rust"), false), (Any, Contains, s("steel"), false)]);
    assert_eq!(terms(r#"!"old rust""#), [(Any, Exact, s("old rust"), false)]);
    assert_eq!(terms(r#""say \"hi\"""#), [(Any, Contains, s("say \"hi\""), false)]);
    assert_eq!(terms(r#"a\ b"#), [(Any, Contains, s("a b"), false)]);
    assert_eq!(terms("rust, steel"), [(Any, Contains, s("rust"), false), (Any, Contains, s("steel"), false)]);
    assert_eq!(terms("-tag:!old"), [(Tag, Exact, s("old"), true)]);
    assert_eq!(terms("!-name:x"), [(Name, Exact, s("x"), true)]);
    assert_eq!(terms("name:\"two words\""), [(Name, Contains, s("two words"), false)]);
    assert_eq!(
        terms("library:starter usage:alpha format:png source:cc0 path:a/b"),
        [
            (Library, Contains, s("starter"), false),
            (Usage, Contains, s("alpha"), false),
            (Format, Contains, s("png"), false),
            (Source, Contains, s("cc0"), false),
            (Path, Contains, s("a/b"), false),
        ]
    );
    // Literal operators and unknown prefixes are plain text.
    assert_eq!(terms(r#""-x" \!y"#), [(Any, Contains, s("-x"), false), (Any, Contains, s("!y"), false)]);
    assert_eq!(terms("ratio:16"), [(Any, Contains, s("ratio:16"), false)]);
    // Odd input never fails.
    assert_eq!(terms(""), []);
    assert_eq!(terms("- ! , name: \"\""), []);
    assert_eq!(terms("\"unclosed phrase"), [(Any, Contains, s("unclosed phrase"), false)]);
    assert_eq!(terms("trailing\\"), [(Any, Contains, s("trailing"), false)]);
}

#[test]
fn queries_match_name_location_tags_library_and_usage() {
    let mut lib = Library::new();
    lib.add_root(LibraryRoot::in_memory("session", "This session", RootKind::Session), &Keep::default());
    let mut steel = Asset::new("materials/metal/Brushed steel.mca", Format::Graph, Usage::Material);
    steel.tags = vec!["Metal".into(), "clean".into()];
    let mut rust = Asset::new("smart-materials/old rust.mca", Format::Graph, Usage::SmartMaterial);
    rust.tags = vec!["metal".into(), "worn".into()];
    rust.provenance = Provenance::Generator { generator: "rust".into(), preset: None };
    let moss = Asset::new("textures/nature/moss.png", Format::Png, Usage::Texture);
    lib.set_assets(&RootId::new("session"), [steel, rust, moss]);
    let cat = lib.catalogue();
    let names = |q: &str| cat.find(&Search::text(q)).iter().map(|h| h.asset.name.clone()).collect::<Vec<_>>();

    assert_eq!(names("STEEL"), ["Brushed steel"]);
    assert_eq!(names("metal"), ["Brushed steel", "old rust"]); // folder or tag
    assert_eq!(names("!metal"), ["Brushed steel", "old rust"]); // folder name or tag, exactly
    assert_eq!(names("!steel"), Vec::<String>::new()); // no field is exactly "steel"
    assert_eq!(names("!\"brushed steel\""), ["Brushed steel"]);
    assert_eq!(names("tag:metal -tag:worn"), ["Brushed steel"]);
    assert_eq!(names("usage:material"), ["Brushed steel", "old rust"]); // contains: smart-material too
    assert_eq!(names("usage:!material"), ["Brushed steel"]);
    assert_eq!(names("usage:!materials"), ["Brushed steel"]); // plural folder names work too
    assert_eq!(names("library:session nature"), ["moss"]);
    assert_eq!(names("library:!this"), Vec::<String>::new());
    assert_eq!(names("source:generator"), ["old rust"]);
    assert_eq!(names("path:!nature"), ["moss"]);
    assert_eq!(names("path:materials"), ["Brushed steel", "old rust"]);
    assert_eq!(names("-path:materials"), ["moss"]);
    assert_eq!(names("moss.png"), ["moss"]); // file name

    let saved = SavedSearch {
        name: "Worn metals".into(),
        usages: SavedSearch::usages_from_names(["smart materials", "nope"]).0,
        location: Some(Location::root(RootId::new("session"))),
        query: "metal".into(),
    };
    assert_eq!(SavedSearch::usages_from_names(["smart materials", "nope"]).1, ["nope"]);
    assert_eq!(cat.find(&saved.compile()).iter().map(|h| h.asset.name.as_str()).collect::<Vec<_>>(), ["old rust"]);
}

#[test]
fn roots_keep_the_editors_order_and_layouts_are_created_only_where_writable() {
    let a = scratch("root-a");
    let b = scratch("root-b");
    write(&a, "materials/a.mca", "g");
    write(&b, "materials/b.mca", "g");
    let sink = Keep::default();
    let ra = LibraryRoot::folder("a", "Starter", RootKind::Starter, &a);
    let rb = LibraryRoot::folder("b", "User", RootKind::User, &b);
    let mut lib = Library::with_roots([ra.clone(), rb.clone()], &sink);
    let order = |lib: &Library| lib.catalogue().find(&Search::all()).iter().map(|h| h.asset.name.clone()).collect::<Vec<_>>();
    assert_eq!(order(&lib), ["a", "b"]);
    lib.set_roots(vec![rb.clone(), ra.clone()], &sink);
    assert_eq!(order(&lib), ["b", "a"]);
    lib.set_roots(vec![rb.clone()], &sink);
    assert_eq!(order(&lib), ["b"]);
    write(&a, "materials/a2.mca", "g");
    assert!(lib.poll(&sink).is_empty(), "removed roots are no longer watched");

    assert!(create_standard_layout(&ra).is_err(), "the starter library is read-only");
    let created = create_standard_layout(&rb).unwrap();
    assert_eq!(created.len(), Usage::ALL.len() - 1, "materials/ existed already");
    assert!(create_standard_layout(&rb).unwrap().is_empty());
    let refresh = lib.poll(&sink);
    assert!(refresh.assets.is_empty(), "empty folders are no assets");
    assert_eq!(refresh.folders, [RootId::new("b")], "but the folder tree changed");
    assert!(lib.catalogue().folders(&RootId::new("b")).any(|f| f == "environments"));

    // A missing library folder is reported once, not on every poll.
    let gone = scratch("root-gone");
    fs::remove_dir_all(&gone).unwrap();
    lib.add_root(LibraryRoot::folder("g", "Gone", RootKind::Extra, &gone), &sink);
    assert_eq!(sink.take().len(), 1);
    lib.poll(&sink);
    lib.poll(&sink);
    assert!(sink.take().is_empty());
    assert_eq!(Listing::read(&gone, &Keep::default()).len(), 0);
    fs::remove_dir_all(a).unwrap();
    fs::remove_dir_all(b).unwrap();
}
