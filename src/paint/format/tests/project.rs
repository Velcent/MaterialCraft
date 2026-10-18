//! Project settings (`project`, since 0.17).

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::json;
use crate::paint::format::FormatError;
use crate::paint::{NormalMapFormat, ProjectSettings};
use crate::zip::ZipReader;
use std::io::Cursor;

fn manifest(bytes: &[u8]) -> json::Value {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    json::parse(std::str::from_utf8(&zip.read("document.json").unwrap()).unwrap()).unwrap()
}

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
fn project_settings_round_trip() {
    let mut doc = full_document();
    doc.project = ProjectSettings { normal_map_format: NormalMapFormat::DirectX, tangent_per_fragment: true, fill_physical_size: true };
    let bytes = to_bytes(&doc);
    let v = manifest(&bytes);
    assert_eq!(v.get("project").and_then(|p| p.get("normal_map_format")).and_then(json::Value::as_str), Some("directx"));
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back.project, doc.project);
    assert_eq!(to_bytes(&back), bytes, "load → save reproduces the file");
}

#[test]
fn defaults_are_not_written_and_missing_means_defaults() {
    let doc = full_document();
    let bytes = to_bytes(&doc);
    assert!(manifest(&bytes).get("project").is_none());
    assert_eq!(from_bytes(&bytes).unwrap().project, ProjectSettings::default());
}

#[test]
fn unknown_fields_are_kept_and_bad_values_refused() {
    let bytes = to_bytes(&full_document());
    let future = edited(&bytes, |v| {
        v.insert("project", json::Value::object().with("tangent_per_fragment", true).with("udim_layout", "tiles"));
    });
    let doc = from_bytes(&future).unwrap();
    assert!(doc.project.tangent_per_fragment);
    let again = manifest(&to_bytes(&doc));
    assert_eq!(again.get("project").and_then(|p| p.get("udim_layout")).and_then(json::Value::as_str), Some("tiles"), "unknown field kept");
    let bad = edited(&bytes, |v| {
        v.insert("project", json::Value::object().with("normal_map_format", "metal"));
    });
    assert!(matches!(from_bytes(&bad), Err(FormatError::Manifest { .. })));
}
