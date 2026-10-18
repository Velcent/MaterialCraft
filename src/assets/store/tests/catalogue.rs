//! The catalogue follows the store at once, without waiting for the watcher, and the watcher's next poll then
//! reports nothing new.

use std::fs;

use super::*;
use crate::assets::{AssetChange, AssetStore, ChangeKind, Library, MetaEdit, RootId, StoreRequest, Thumbnail};
use crate::diag::NullSink;

fn change(root: &str, path: &str, kind: ChangeKind) -> AssetChange {
    AssetChange { root: RootId::new(root), path: path.into(), kind }
}

#[test]
fn every_request_shows_in_the_catalogue_at_once() {
    let (a, b) = (temp_dir("cat-a"), temp_dir("cat-b"));
    let sink = NullSink;
    let store = AssetStore::new();
    let mut lib = Library::with_roots([user_root(&a), extra_root(&b)], &sink);
    let user = RootId::new("user");

    // Create (through the store directly), then apply.
    let created = store.create(&user_root(&a), "alphas/a.png", &full(&sidecar("A"))).unwrap();
    let refresh = lib.apply(&created, &sink);
    assert_eq!(refresh.assets, [change("user", "alphas/a.png", ChangeKind::Added)]);
    assert_eq!(refresh.folders, std::slice::from_ref(&user), "the new folder shows in the tree");
    let asset = lib.catalogue().get(&user, "alphas/a.png").unwrap();
    assert_eq!((asset.name.as_str(), &asset.thumbnail), ("A", &Thumbnail::File("alphas/a.png.thumb.png".into())));
    assert!(lib.poll(&sink).is_empty(), "the watcher saw it already");

    let run = |lib: &mut Library, r: StoreRequest| lib.run(&store, &r, &sink).unwrap().1;
    let edit = MetaEdit { tags: Some(vec!["rough".into()]), ..MetaEdit::default() };
    let refresh = run(&mut lib, StoreRequest::Update { root: user.clone(), path: "alphas/a.png".into(), edit });
    assert_eq!(refresh.assets, [change("user", "alphas/a.png", ChangeKind::Modified)]);
    assert_eq!(lib.catalogue().get(&user, "alphas/a.png").unwrap().tags, ["rough"]);

    let refresh = run(&mut lib, StoreRequest::Rename { root: user.clone(), path: "alphas/a.png".into(), name: "Bee".into() });
    assert_eq!(refresh.assets, [change("user", "alphas/Bee.png", ChangeKind::Added), change("user", "alphas/a.png", ChangeKind::Removed)]);
    assert_eq!(lib.catalogue().get(&user, "alphas/Bee.png").unwrap().name, "Bee");
    assert!(lib.poll(&sink).is_empty());

    let to = RootId::new("extra");
    let refresh = run(&mut lib, StoreRequest::Move { root: user.clone(), path: "alphas/Bee.png".into(), to: to.clone(), folder: "worn".into() });
    assert_eq!(refresh.assets, [change("user", "alphas/Bee.png", ChangeKind::Removed), change("extra", "worn/Bee.png", ChangeKind::Added)]);
    let moved = lib.catalogue().get(&to, "worn/Bee.png").unwrap();
    assert_eq!(
        (moved.name.as_str(), moved.tags.as_slice(), &moved.thumbnail),
        ("Bee", &["rough".to_string()][..], &Thumbnail::File("worn/Bee.png.thumb.png".into()))
    );
    assert!(lib.catalogue().get(&user, "alphas/Bee.png").is_none());

    let refresh = run(&mut lib, StoreRequest::Delete { root: to.clone(), path: "worn/Bee.png".into() });
    assert_eq!(refresh.assets, [change("extra", "worn/Bee.png", ChangeKind::Removed)]);
    assert!(lib.catalogue().is_empty());
    assert!(lib.poll(&sink).is_empty());

    // A failed request changes nothing and says why; an unknown library is refused.
    let missing = lib.run(&store, &StoreRequest::Delete { root: user.clone(), path: "alphas/none.png".into() }, &sink);
    assert!(missing.is_err());
    let unknown = lib.run(&store, &StoreRequest::Delete { root: RootId::new("nope"), path: "a.png".into() }, &sink);
    assert!(unknown.unwrap_err().to_string().contains("nope"));
    fs::remove_dir_all(&a).unwrap();
    fs::remove_dir_all(&b).unwrap();
}
