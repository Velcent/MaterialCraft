//! Fields from a newer minor version survive load → save.

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::json::{self, Value};
use crate::paint::format::FORMAT_VERSION;
use crate::paint::format::manifest::extras::Extras;
use crate::zip::ZipReader;
use std::io::Cursor;

fn manifest(bytes: &[u8]) -> Value {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    json::parse(std::str::from_utf8(&zip.read("document.json").unwrap()).unwrap()).unwrap()
}

#[test]
fn unknown_fields_are_kept() {
    let original = to_bytes(&full_document());
    let future = rewrite(&original, |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        v.insert("format_version", "0.18");
        v.insert("udims", vec![1001, 1002]);
        let set = &mut v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0];
        set.insert("color_profile", "ACEScg");
        let layer = &mut set.get_mut("layers").unwrap().as_array_mut().unwrap()[1];
        layer.insert("locked", true);
        layer.get_mut("mask").unwrap().insert("invert", true);
        Some(v.to_string_pretty().into_bytes())
    });
    let doc = from_bytes(&future).unwrap();
    assert!(doc.foreign_file(Extras::PATH).is_some());
    let saved = to_bytes(&doc);
    let v = manifest(&saved);
    assert_eq!(v["format_version"].as_str(), Some(FORMAT_VERSION));
    assert_eq!(v["udims"].to_string(), "[1001,1002]");
    let set = &v["texture_sets"][0];
    assert_eq!(set["color_profile"].as_str(), Some("ACEScg"));
    assert_eq!(set["layers"][1]["locked"].as_bool(), Some(true));
    assert_eq!(set["layers"][1]["mask"]["invert"].as_bool(), Some(true));
    // Stable from here on.
    assert_eq!(from_bytes(&saved).unwrap(), doc);
    assert_eq!(to_bytes(&from_bytes(&saved).unwrap()), saved);
    // A file without unknown fields gets no extras file.
    assert!(from_bytes(&original).unwrap().foreign_file(Extras::PATH).is_none());
}
