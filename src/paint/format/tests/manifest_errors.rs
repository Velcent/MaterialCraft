//! Manifests that break the spec are rejected with the JSON path of the problem.

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::json::{self, Value};
use crate::paint::format::FormatError;

/// Loads the full document after editing its manifest.
fn load_edited(edit: impl Fn(&mut Value)) -> Result<crate::paint::Document, FormatError> {
    let bytes = rewrite(&to_bytes(&full_document()), |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        edit(&mut v);
        Some(v.to_string_pretty().into_bytes())
    });
    from_bytes(&bytes)
}

fn layer(v: &mut Value, i: usize) -> &mut Value {
    v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0]
        .get_mut("layers")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .get_mut(i)
        .unwrap()
}

fn manifest_error_at(r: Result<crate::paint::Document, FormatError>) -> String {
    match r {
        Err(FormatError::Manifest { at, .. }) => at,
        other => panic!("expected a manifest error, got {other:?}"),
    }
}

#[test]
fn version_rules() {
    let newer = load_edited(|v| {
        v.insert("format_version", "1.0");
    });
    assert!(matches!(newer, Err(FormatError::NewerVersion { found, .. }) if found == "1.0"));
    let msg = load_edited(|v| {
        v.insert("format_version", "2.3");
    })
    .unwrap_err()
    .to_string();
    assert!(msg.contains("2.3") && msg.contains("newer"), "{msg}");
    assert!(load_edited(|v| {
        v.insert("format_version", "0.9");
    })
    .is_ok());
    assert!(matches!(
        load_edited(|v| {
            v.insert("format", "other");
        }),
        Err(FormatError::NotMcpaint)
    ));
}

#[test]
fn bad_fields_name_their_path() {
    let at = manifest_error_at(load_edited(|v| {
        layer(v, 0).insert("kind", "filter");
    }));
    assert_eq!(at, "texture_sets[0].layers[0].kind");
    let at = manifest_error_at(load_edited(|v| {
        layer(v, 0).insert("opacity", "high");
    }));
    assert_eq!(at, "texture_sets[0].layers[0].opacity");
    let at = manifest_error_at(load_edited(|v| {
        let b = layer(v, 0).get_mut("blend").unwrap();
        b.get_mut("base_color").unwrap().insert("mode", "glow");
    }));
    assert_eq!(at, "texture_sets[0].layers[0].blend.base_color.mode");
    let at = manifest_error_at(load_edited(|v| {
        let img = layer(v, 1)
            .get_mut("images")
            .unwrap()
            .get_mut("base_color")
            .unwrap();
        img.insert("format", "rgb12");
    }));
    assert_eq!(at, "texture_sets[0].layers[1].images.base_color.format");
    let at = manifest_error_at(load_edited(|v| {
        v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0]
            .insert("resolution", vec![0, 5]);
    }));
    assert_eq!(at, "texture_sets[0].resolution");
}

#[test]
fn declarations_must_match_images() {
    // A declared size that differs from the stored image (baked maps may have any size).
    let r = load_edited(|v| {
        let set = &mut v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0];
        let baked = &mut set.get_mut("baked").unwrap().as_array_mut().unwrap()[0];
        baked
            .get_mut("image")
            .unwrap()
            .insert("size", vec![151, 130]);
    });
    assert!(matches!(r, Err(FormatError::Image { .. })), "{r:?}");
    // Layer images must match the texture set: caught before decoding.
    let r = load_edited(|v| {
        let img = layer(v, 1)
            .get_mut("images")
            .unwrap()
            .get_mut("base_color")
            .unwrap();
        img.insert("size", vec![151, 130]);
    });
    assert!(matches!(r, Err(FormatError::Invalid(_))), "{r:?}");
    // A declared format that differs from the stored image.
    let r = load_edited(|v| {
        let img = layer(v, 1)
            .get_mut("images")
            .unwrap()
            .get_mut("base_color")
            .unwrap();
        img.insert("format", "rgba16");
    });
    assert!(matches!(r, Err(FormatError::Image { .. })), "{r:?}");
    // A layer whose images break document invariants is caught by validation after loading.
    let r = load_edited(|v| {
        layer(v, 2).insert("id", "p0");
    });
    assert!(matches!(r, Err(FormatError::Invalid(_))), "{r:?}");
}
