use super::ops;
use super::*;
use crate::bake::{AoSettings, Attenuation, BakeSettings, BakedMaps, MapKind, MapSelection, ThicknessSettings};
use crate::history::PaintHistory;
use crate::paint::edit::{Edit, Transaction};
use crate::paint::{BakedKind, Document, TextureSet};
use crate::tex::Tex;

fn doc() -> Document {
    let mut d = Document::default();
    for id in ["a", "b", "c"] {
        d.texture_sets.push(TextureSet::new(id, id.to_uppercase(), (256, 256)));
    }
    d
}

fn ids(d: &Document) -> Vec<String> {
    d.texture_sets.iter().map(|s| s.id.clone()).collect()
}

fn bake(d: &Document, id: &str) -> SetBakeConfig {
    d.texture_set(id).unwrap().bake.clone()
}

fn ao(rays: u32) -> MapSettings {
    MapSettings::AmbientOcclusion { rays, distance: 0.25, ao: AoSettings { attenuation: Attenuation::Linear, ..AoSettings::default() } }
}

fn run(d: &mut Document, op: impl FnOnce(&mut Transaction) -> Result<(), crate::paint::edit::EditError>) -> Vec<Edit> {
    let mut tx = Transaction::new(d);
    op(&mut tx).unwrap();
    tx.commit()
}

#[test]
fn settings_round_trip_through_the_configuration() {
    let mut s = BakeSettings { size: 512, max_dist: 0.02, rear_dist: Some(0.01), ao_rays: 17, ao_dist: 0.3, padding: 3, samples: 2, ..BakeSettings::default() };
    s.maps = MapSelection::of(&[MapKind::Normal, MapKind::Thickness]);
    s.ignore_backfaces = false;
    s.thickness = ThicknessSettings { rays: 9, maximise_range: false, ..ThicknessSettings::default() };
    let c = SetBakeConfig::from_settings(&s);
    assert_eq!(c.settings(), s);
    assert_eq!(c.included(), s.maps);
    let only_ao = c.settings_for(MapSelection::of(&[MapKind::AmbientOcclusion]));
    assert_eq!((only_ao.ao_rays, only_ao.maps.kinds().count()), (17, 1));
    assert_eq!(TextureSet::new("x", "X", (2048, 512)).bake.common.size, 2048);
    assert!(SetBakeConfig::default().common.high.low_as_high);
}

#[test]
fn linked_settings_change_in_every_set() {
    let mut d = doc();
    run(&mut d, |tx| ops::set_map_settings(tx, "a", &ao(5)));
    assert_eq!(bake(&d, "a").maps.get(MapKind::AmbientOcclusion).settings(), &ao(5));
    assert_ne!(bake(&d, "b").maps.get(MapKind::AmbientOcclusion).settings(), &ao(5), "unlinked: only a");
    // Linking from b makes every set take b's AO settings.
    run(&mut d, |tx| ops::set_link(tx, "b", BakePart::Map(MapKind::AmbientOcclusion), true));
    assert!(d.bake_links.maps.contains(MapKind::AmbientOcclusion));
    assert_eq!(bake(&d, "a").maps.get(MapKind::AmbientOcclusion), bake(&d, "b").maps.get(MapKind::AmbientOcclusion));
    run(&mut d, |tx| ops::set_map_settings(tx, "c", &ao(99)));
    assert!(ids(&d).iter().all(|id| bake(&d, id).maps.get(MapKind::AmbientOcclusion).settings() == &ao(99)));
    // Common is not linked: a size change stays in one set.
    let mut common = bake(&d, "a").common;
    common.size = 64;
    run(&mut d, |tx| ops::set_common(tx, "a", &common));
    assert_eq!((bake(&d, "a").common.size, bake(&d, "b").common.size), (64, 256));
    run(&mut d, |tx| ops::set_link(tx, "a", BakePart::Common, true));
    assert!(ids(&d).iter().all(|id| bake(&d, id).common.size == 64));
}

#[test]
fn copies_and_checks() {
    let mut d = doc();
    run(&mut d, |tx| ops::set_map_settings(tx, "a", &ao(3)));
    let to = vec!["b".to_string()];
    run(&mut d, |tx| ops::copy_part(tx, "a", BakePart::Map(MapKind::AmbientOcclusion), &to));
    assert_eq!(bake(&d, "b").maps.get(MapKind::AmbientOcclusion).settings(), &ao(3));
    assert_ne!(bake(&d, "c").maps.get(MapKind::AmbientOcclusion).settings(), &ao(3));
    assert!(d.bake_links.maps.is_empty(), "a copy links nothing");
    let mut common = bake(&d, "a").common;
    common.padding = 1;
    run(&mut d, |tx| ops::set_common(tx, "a", &common));
    let all = ids(&d);
    run(&mut d, |tx| ops::copy_all(tx, "a", &all));
    assert!(ids(&d).iter().all(|id| bake(&d, id).common.padding == 1 && bake(&d, id).maps.get(MapKind::AmbientOcclusion).settings() == &ao(3)));

    run(&mut d, |tx| ops::check_all(tx, "a", false));
    assert!(bake(&d, "a").included().is_empty());
    let flags = MapFlags { include: true, show: true, auto_rebake: true };
    run(&mut d, |tx| ops::set_flags(tx, "a", MapKind::Curvature, flags));
    assert_eq!(bake(&d, "a").shown(), Some(MapKind::Curvature));
    run(&mut d, |tx| ops::show_map(tx, "a", Some(MapKind::Id)));
    assert_eq!(bake(&d, "a").shown(), Some(MapKind::Id));
    run(&mut d, |tx| ops::invert_checks(tx, "a"));
    assert_eq!(bake(&d, "a").included().kinds().count(), 6);
    assert!(!bake(&d, "a").included().contains(MapKind::Curvature));
    run(&mut d, |tx| ops::copy_checks(tx, "a", &all));
    assert!(ids(&d).iter().all(|id| bake(&d, id).included() == bake(&d, "a").included()));
    // Nothing to change: no edit.
    assert!(run(&mut d, |tx| ops::copy_checks(tx, "a", &all)).is_empty());
    assert!(Transaction::new(&mut d).apply(Edit::SetBakeConfig { set: "zz".into(), config: Box::default() }).is_err());
}

#[test]
fn edits_undo_through_the_history() {
    let mut d = doc();
    let before = d.clone();
    let mut history = PaintHistory::default();
    let mut tx = Transaction::new(&mut d);
    ops::set_link(&mut tx, "a", BakePart::Map(MapKind::Thickness), true).unwrap();
    ops::set_map_settings(&mut tx, "b", &MapSettings::Thickness(ThicknessSettings { rays: 3, ..ThicknessSettings::default() })).unwrap();
    let inverses = tx.commit();
    history.record_all(&d, "Bake settings", None, inverses);
    let mut maps = BakedMaps::default();
    maps.maps.insert(MapKind::AmbientOcclusion, Tex::solid(32, 32, [0.5, 0.5, 0.5, 1.0]));
    let baked = to_baked(&maps).unwrap();
    history.apply(&mut d, "Bake", None, Edit::SetBaked { set: "a".into(), maps: baked.clone() }).unwrap();
    let mut tx = Transaction::new(&mut d);
    ops::store_baked(&mut tx, "a", baked).unwrap();
    drop(tx.commit());
    assert_eq!(d.texture_set("a").unwrap().baked.len(), 1, "same kind replaced");
    let ao = d.texture_set("a").unwrap().baked(&BakedKind::AmbientOcclusion).unwrap();
    assert!((ao.get(3, 3)[0] - 0.5).abs() < 1e-3);
    history.undo(&mut d).unwrap();
    history.undo(&mut d).unwrap();
    assert_eq!(d, before);
}

#[test]
fn keys_and_kinds() {
    let k = SettingKey::from_baker("Body @ 2", "max_dist");
    assert_eq!(k.to_string(), "common.max_dist@Body @ 2");
    assert_eq!(SettingKey::parse(&k.to_string()), Some(k.clone()));
    assert_eq!(k.map(), None);
    let ao = SettingKey::from_baker("s", "ambient_occlusion.rays");
    assert_eq!(ao.map(), Some(MapKind::AmbientOcclusion));
    assert_eq!(SettingKey::parse("max_dist"), None);
    for kind in MapKind::ALL {
        assert_eq!(map_kind(&baked_kind(kind)), Some(kind));
    }
    assert_eq!(map_kind(&BakedKind::Other("bent".into())), None);
    let p = HighPlacement { center: [1.0, 2.0, 3.0], radius: 2.0 };
    assert_eq!(p.apply([3.0, 2.0, 1.0]), [1.0, 0.0, -1.0]);
}
