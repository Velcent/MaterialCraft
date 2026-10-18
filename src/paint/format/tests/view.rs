//! The document's saved view (`view`, since 0.12).

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::camera::{CameraSettings, DepthOfField, Orbit, Projection};
use crate::json;
use crate::paint::saved_view::{SavedCamera, SavedEnvironment, SavedView};
use crate::zip::ZipReader;
use std::io::Cursor;

fn view() -> SavedView {
    let close = SavedCamera {
        name: "Close-up".into(),
        orbit: Orbit { yaw: -1.25, pitch: 0.5, distance: 2.0, pan: [0.125, -0.25], roll: 0.0 },
        settings: CameraSettings {
            projection: Projection::Orthographic,
            fov_y: 0.5,
            depth_of_field: DepthOfField { enabled: true, focus_distance: 1.75, aperture: 0.05 },
        },
    };
    SavedView {
        cameras: vec![SavedCamera::default(), close],
        active_camera: 1,
        exposure_stops: -0.75,
        environment: SavedEnvironment { light_yaw: 0.25, light_follows_camera: false, ambient: 0.5, ..SavedEnvironment::default() },
    }
}

fn manifest(bytes: &[u8]) -> json::Value {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    json::parse(std::str::from_utf8(&zip.read("document.json").unwrap()).unwrap()).unwrap()
}

/// `document.json` of `bytes` changed by `edit`.
fn edited(bytes: &[u8], edit: impl Fn(&mut json::Value)) -> Vec<u8> {
    rewrite(bytes, |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        edit(&mut v);
        Some(v.to_string_pretty().into_bytes())
    })
}

#[test]
fn the_view_round_trips_exactly() {
    let mut doc = full_document();
    assert_eq!(from_bytes(&to_bytes(&doc)).unwrap().view, None, "no view, no field");
    doc.view = Some(view());
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back.view, doc.view);
    assert_eq!(to_bytes(&back), bytes);
}

#[test]
fn unknown_view_fields_are_kept_and_a_broken_view_is_dropped() {
    let mut doc = full_document();
    doc.view = Some(view());
    let original = to_bytes(&doc);
    let future = edited(&original, |v| {
        let view = v.get_mut("view").unwrap();
        view.insert("grid", true);
        view.get_mut("cameras").unwrap().as_array_mut().unwrap()[1].insert("locked", true);
    });
    let back = from_bytes(&future).unwrap();
    assert_eq!(back.view, doc.view);
    let saved = to_bytes(&back);
    let again = from_bytes(&saved).unwrap();
    assert_eq!(again, back, "kept fields are stable");
    let m = manifest(&saved);
    assert_eq!(m["view"]["grid"].as_bool(), Some(true));
    assert_eq!(m["view"]["cameras"][1]["locked"].as_bool(), Some(true));
    // an unknown projection drops the view, not the document
    let broken = edited(&original, |v| {
        v.get_mut("view").unwrap().get_mut("cameras").unwrap().as_array_mut().unwrap()[0].insert("projection", "fisheye");
    });
    let doc2 = from_bytes(&broken).unwrap();
    assert_eq!(doc2.view, None);
    assert_eq!(doc2.texture_sets, doc.texture_sets);
}
