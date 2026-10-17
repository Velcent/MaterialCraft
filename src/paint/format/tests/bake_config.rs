//! Bake configurations and link flags round-trip; files without them (before 0.5) read as the defaults.

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::bake::{CurvatureRange, CurvatureSettings, MapKind, MapSelection, PositionBounds, PositionMode, PositionSettings, TangentSource};
use crate::json;
use crate::paint::bake_config::{BakeLinks, HighPlacement, HighPoly, MapFlags, MapSettings, SetBakeConfig};
use crate::paint::format::FormatError;

#[test]
fn bake_configuration_round_trips() {
    let mut doc = full_document();
    doc.bake_links = BakeLinks { common: true, maps: MapSelection::of(&[MapKind::Curvature]) };
    let c = &mut doc.texture_sets[0].bake;
    c.common.rear_dist = Some(0.0125);
    c.common.high =
        HighPoly { low_as_high: false, files: vec!["Vase HP.obj".into()], placement: Some(HighPlacement { center: [0.1, -2.0, 3.5], radius: 7.25 }) };
    c.maps.set_settings(MapSettings::Normal { tangents: TangentSource::Generated });
    c.maps.set_settings(MapSettings::Curvature(CurvatureSettings { range: CurvatureRange::Fixed(12.5), ..CurvatureSettings::default() }));
    c.maps.set_settings(MapSettings::Position(PositionSettings { mode: PositionMode::PerAxis, bounds: PositionBounds::High }));
    *c.maps.flags_mut(MapKind::Id) = MapFlags { include: false, show: true, auto_rebake: true };
    let back = from_bytes(&to_bytes(&doc)).unwrap();
    assert_eq!(back.bake_links, doc.bake_links);
    assert_eq!(back.texture_sets[0].bake, doc.texture_sets[0].bake);
    assert_eq!(back, doc);
}

fn edit_manifest(bytes: &[u8], f: impl Fn(String) -> String) -> Vec<u8> {
    rewrite(bytes, |name, data| if name == "document.json" { Some(f(String::from_utf8(data).unwrap()).into_bytes()) } else { Some(data) })
}

#[test]
fn older_files_get_defaults_and_bad_values_are_errors() {
    let doc = full_document();
    let bytes = to_bytes(&doc);
    let old = edit_manifest(&bytes, |text| {
        let mut v = json::parse(&text).unwrap();
        v.insert("format_version", "0.4");
        v.remove("bake_links");
        v.get_mut("texture_sets").unwrap().as_array_mut().unwrap().iter_mut().for_each(|s| drop(s.remove("bake")));
        v.to_string()
    });
    let back = from_bytes(&old).unwrap();
    let set = &back.texture_sets[0];
    assert_eq!(set.bake, SetBakeConfig::with_size(set.resolution.0.max(set.resolution.1)));
    assert_eq!(back.bake_links, BakeLinks::default());
    let bad = edit_manifest(&bytes, |text| {
        let mut v = json::parse(&text).unwrap();
        let set = &mut v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0];
        let ao = set.get_mut("bake").unwrap().get_mut("maps").unwrap().get_mut("ambient_occlusion").unwrap();
        ao.insert("attenuation", "loud");
        v.to_string()
    });
    assert!(matches!(from_bytes(&bad), Err(FormatError::Manifest { .. })), "{:?}", from_bytes(&bad).err());
}
