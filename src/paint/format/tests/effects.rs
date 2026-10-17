//! Format 0.3: effect stacks, mask states and geometry masks; 0.2 image masks still load; unknown effect kinds
//! load as not computable and are written back unchanged.

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::json::{self, Value};
use crate::paint::format::FormatError;
use crate::paint::{Channel, CompositeWarning, Compositor, Document, EffectKind, Mask};
use crate::zip::ZipReader;
use std::io::Cursor;

/// Rewrites the manifest of `bytes`.
fn edit_manifest(bytes: &[u8], edit: impl Fn(&mut Value)) -> Vec<u8> {
    rewrite(bytes, |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        edit(&mut v);
        Some(v.to_string_pretty().into_bytes())
    })
}

fn manifest(bytes: &[u8]) -> Value {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    json::parse(std::str::from_utf8(&zip.read("document.json").unwrap()).unwrap()).unwrap()
}

fn layers(v: &mut Value) -> &mut Vec<Value> {
    v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0].get_mut("layers").unwrap().as_array_mut().unwrap()
}

fn layer<'v>(v: &'v mut Value, id: &str) -> &'v mut Value {
    layers(v).iter_mut().find(|l| l["id"].as_str() == Some(id)).unwrap()
}

/// The full document without the layers that use 0.3 features.
fn document_0_2() -> Document {
    let mut doc = full_document();
    doc.texture_sets[0].layers.retain(|l| l.effects.is_empty() && l.geometry_mask.is_none());
    doc
}

#[test]
fn version_0_2_image_masks_load_as_painted_stacks() {
    let doc = document_0_2();
    let old = edit_manifest(&to_bytes(&doc), |v| {
        v.insert("format_version", "0.2");
        for l in layers(v).iter_mut() {
            let Some(m) = l.get_mut("mask") else { continue };
            let image = m["effects"][0]["image"].clone();
            *m = Value::object().with("kind", "image").with("image", image.with("dpi", 72));
        }
    });
    assert_eq!(manifest(&old)["texture_sets"][0]["layers"][1]["mask"]["kind"].as_str(), Some("image"));
    let back = from_bytes(&old).unwrap();
    let masked = back.texture_sets[0].layer("p0").unwrap();
    assert_eq!(masked.mask.as_ref().map(|m| (m.enabled, m.initial, m.effects.len())), Some((true, 0.0, 1)));
    let mut expected = doc.clone();
    expected.foreign = back.foreign.clone();
    assert_eq!(back, expected, "0.2 masks become black masks with the image painted on");
    // Re-saved as 0.3: the unknown field of the 0.2 image reference moved along.
    let v = manifest(&to_bytes(&back));
    let m = &v["texture_sets"][0]["layers"][1]["mask"];
    assert_eq!(m["kind"].as_str(), Some("stack"));
    assert_eq!(m["effects"][0]["image"]["dpi"].as_u64(), Some(72));
}

#[test]
fn every_effect_kind_is_written_and_kept() {
    let doc = full_document();
    let v = manifest(&to_bytes(&doc));
    let l = &v["texture_sets"][0]["layers"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some("effects")).unwrap().clone();
    let kinds: Vec<&str> = l["effects"].as_array().unwrap().iter().map(|e| e["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["paint", "fill", "fill", "generator", "filter", "levels", "color_selection", "polygon_fill", "anchor", "reference", "warp3d"]);
    assert_eq!(l["mask"]["kind"].as_str(), Some("stack"));
    assert_eq!(l["mask"]["enabled"].as_bool(), Some(false));
    assert_eq!(l["mask"]["effects"][1]["value"].as_f64(), Some(0.3));
    assert_eq!(l["geometry_mask"]["parts"].to_string(), r#"["body","trim 2"]"#);
    assert_eq!(l["effects"][3]["params"]["seed"].as_str(), Some("7"));
    assert_eq!(l["effects"][10]["nested"]["a"].as_str(), Some("b"), "unknown effects are written as read");
}

#[test]
fn unknown_effect_kinds_load_as_not_computable_and_round_trip() {
    let doc = document_0_2();
    let bytes = edit_manifest(&to_bytes(&doc), |v| {
        let future = json::parse(r#"{"kind": "hologram", "blend": {"roughness": {"mode": "normal"}}, "beams": [1, 2, 3]}"#).unwrap();
        let fill = layer(v, "fill");
        fill.insert("effects", vec![future.clone()]);
        let p0 = layer(v, "p0");
        p0.get_mut("mask").unwrap().get_mut("effects").unwrap().push(future);
    });
    let back = from_bytes(&bytes).unwrap();
    let set = &back.texture_sets[0];
    let fill = set.layer("fill").unwrap();
    assert!(matches!(&fill.effects[0].kind, EffectKind::Unknown(u) if u.kind == "hologram"));
    let warnings = Compositor::new(set).warnings(Channel::BaseColor);
    assert!(
        warnings.iter().any(|w| matches!(w, CompositeWarning::EffectNotComputable { layer, in_mask: true, kind, .. } if layer == "p0" && kind == "hologram")),
        "{warnings:?}"
    );
    let saved = to_bytes(&back);
    let v = manifest(&saved);
    let written = &v["texture_sets"][0]["layers"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some("fill")).unwrap()["effects"][0];
    assert_eq!(written.to_string(), r#"{"kind":"hologram","blend":{"roughness":{"mode":"normal"}},"beams":[1,2,3]}"#);
    assert_eq!(from_bytes(&saved).unwrap(), back);
}

#[test]
fn unknown_fields_inside_effects_are_kept() {
    let bytes = edit_manifest(&to_bytes(&full_document()), |v| {
        let l = layer(v, "effects");
        let e = &mut l.get_mut("effects").unwrap().as_array_mut().unwrap()[1];
        e.insert("seed", 42);
        e.get_mut("blend").unwrap().get_mut("roughness").unwrap().insert("clip", true);
        let m = l.get_mut("mask").unwrap();
        m.insert("feather", 0.5);
        m.get_mut("effects").unwrap().as_array_mut().unwrap()[1].get_mut("blend").unwrap().insert("clip", false);
        l.get_mut("geometry_mask").unwrap().insert("hide", true);
    });
    let v = manifest(&to_bytes(&from_bytes(&bytes).unwrap()));
    let l = v["texture_sets"][0]["layers"].as_array().unwrap().iter().find(|l| l["id"].as_str() == Some("effects")).unwrap().clone();
    assert_eq!(l["effects"][1]["seed"].as_u64(), Some(42));
    assert_eq!(l["effects"][1]["blend"]["roughness"]["clip"].as_bool(), Some(true));
    assert_eq!(l["mask"]["feather"].as_f64(), Some(0.5));
    assert_eq!(l["mask"]["effects"][1]["blend"]["clip"].as_bool(), Some(false));
    assert_eq!(l["geometry_mask"]["hide"].as_bool(), Some(true));
}

#[test]
fn broken_effects_name_their_path() {
    let base = to_bytes(&full_document());
    let at = |edit: &dyn Fn(&mut Value)| match from_bytes(&edit_manifest(&base, edit)) {
        Err(FormatError::Manifest { at, .. }) => at,
        other => panic!("expected a manifest error, got {other:?}"),
    };
    let i = layers(&mut manifest(&base)).iter().position(|l| l["id"].as_str() == Some("effects")).unwrap();
    let path = |rest: &str| format!("texture_sets[0].layers[{i}].{rest}");
    let effect = |v: &mut Value, n: usize| -> Value { layer(v, "effects")["effects"][n].clone() };
    let set_effect = |v: &mut Value, n: usize, e: Value| layer(v, "effects").get_mut("effects").unwrap().as_array_mut().unwrap()[n] = e;
    assert_eq!(
        at(&|v| {
            let mut e = effect(v, 1);
            e.get_mut("blend").unwrap().get_mut("roughness").unwrap().insert("mode", "glow");
            set_effect(v, 1, e);
        }),
        path("effects[1].blend.roughness.mode")
    );
    assert_eq!(
        at(&|v| {
            layer(v, "effects").get_mut("effects").unwrap().as_array_mut().unwrap()[7].insert("mode", "lasso");
        }),
        path("effects[7].mode")
    );
    assert_eq!(
        at(&|v| {
            layer(v, "effects").get_mut("effects").unwrap().as_array_mut().unwrap()[8].remove("id");
        }),
        path("effects[8].id")
    );
    assert_eq!(
        at(&|v| {
            layer(v, "effects").get_mut("mask").unwrap().insert("kind", "vector");
        }),
        path("mask.kind")
    );
    assert_eq!(
        at(&|v| {
            layer(v, "effects").insert("geometry_mask", Value::object());
        }),
        path("geometry_mask")
    );
    assert_eq!(
        at(&|v| {
            layer(v, "effects").get_mut("mask").unwrap().get_mut("effects").unwrap().as_array_mut().unwrap()[1].insert("value", "x");
        }),
        path("mask.effects[1].value")
    );
}

#[test]
fn masks_validate_and_constant_masks_need_no_images() {
    let mut doc = document_0_2();
    let l = doc.texture_sets[0].layer_mut("fill").unwrap();
    l.mask = Some(Mask::new(0.25));
    let back = from_bytes(&to_bytes(&doc)).unwrap();
    assert_eq!(back.texture_sets[0].layer("fill").unwrap().mask, Some(Mask::new(0.25)));
    doc.texture_sets[0].layer_mut("fill").unwrap().mask = Some(Mask::new(1.5));
    assert!(matches!(crate::paint::format::save(&doc, Vec::new()), Err(FormatError::Invalid(_))));
}
