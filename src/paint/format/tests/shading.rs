//! Shader settings (`shader_instances`, `mesh_unit`, `texture_sets[].shader`, since 0.13).

use super::corruption::rewrite;
use super::fixtures::full_document;
use super::round_trip::{from_bytes, to_bytes};
use crate::json;
use crate::mesh::LengthUnit;
use crate::paint::format::FormatError;
use crate::shading::{Displacement, DisplacementUnit, ParamValue, ShaderEdit, ShaderFile, ShaderInstance, ShaderSettings, Tessellation, TessellationMode};
use crate::zip::ZipReader;
use std::io::Cursor;

fn settings() -> ShaderSettings {
    let file = ShaderFile::builtin();
    let mut skin = ShaderInstance::new("skin", "Skin", &file);
    skin.set_value("ao_strength", Some(ParamValue::Float(0.3)));
    skin.set_value("sss", Some(ParamValue::Bool(true)));
    skin.set_value("sss_color", Some(ParamValue::Color([1.0, 0.25, 0.125])));
    skin.displacement = Displacement { enabled: true, scale: 1.5, unit: DisplacementUnit::Centimetres, midlevel: 0.5, ..Displacement::default() };
    skin.tessellation = Tessellation { mode: TessellationMode::Uniform { levels: 3 }, smooth: true };
    let mut metal = ShaderInstance::new("metal/../x", "Metal", &file);
    metal.tessellation = Tessellation { mode: TessellationMode::EdgeLength { max_relative: 0.005 }, smooth: false };
    let mut s = ShaderSettings::default();
    ShaderEdit::Insert { at: 0, instance: Box::new(skin), sets: vec!["body".into(), "trim".into()] }.apply(&mut s).unwrap();
    ShaderEdit::Insert { at: 1, instance: Box::new(metal), sets: vec![] }.apply(&mut s).unwrap();
    s.mesh_unit = Some(LengthUnit::Centimetre);
    s
}

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
fn shader_settings_round_trip_exactly() {
    let mut doc = full_document();
    let plain = to_bytes(&doc);
    let m = manifest(&plain);
    assert!(m.get("shader_instances").is_none() && m.get("mesh_unit").is_none(), "nothing is written for documents without shader settings");
    assert!(m["texture_sets"][0].get("shader").is_none());
    doc.shading = settings();
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back.shading, doc.shading);
    assert_eq!(back.shader_instance("trim").map(|i| i.name.as_str()), Some("Skin"), "sets share an instance");
    assert_eq!(to_bytes(&back), bytes);
    let m = manifest(&bytes);
    assert_eq!(m["texture_sets"][0]["shader"].as_str(), Some("skin"));
    assert_eq!(m["mesh_unit"].as_str(), Some("cm"));
    assert_eq!(m["shader_instances"][0]["values"]["ao_strength"].to_string(), "0.3");
}

#[test]
fn unknown_shader_fields_are_kept_and_bad_ones_refused() {
    let mut doc = full_document();
    doc.shading = settings();
    let original = to_bytes(&doc);
    let future = edited(&original, |v| {
        let inst = &mut v.get_mut("shader_instances").unwrap().as_array_mut().unwrap()[0];
        inst.insert("thumbnail", "skin.png");
        inst.get_mut("displacement").unwrap().insert("filter", "smooth");
        inst.get_mut("tessellation").unwrap().insert("adaptive", true);
        // an assignment to an instance that doesn't exist is dropped
        v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[2].insert("shader", "missing");
    });
    let back = from_bytes(&future).unwrap();
    assert_eq!(back.shading, doc.shading);
    let m = manifest(&to_bytes(&back));
    let inst = &m["shader_instances"][0];
    assert_eq!(inst["thumbnail"].as_str(), Some("skin.png"));
    assert_eq!(inst["displacement"]["filter"].as_str(), Some("smooth"));
    assert_eq!(inst["tessellation"]["adaptive"].as_bool(), Some(true));
    for (edit, at) in [
        (r#"{"unit": "furlong"}"#, "shader_instances[0].displacement.unit"),
        (r#"{"mode": "adaptive"}"#, "shader_instances[0].tessellation.mode"),
        (r#"{"values": {"x": "red"}}"#, "shader_instances[0].values"),
        (r#"{"fingerprint": "xyz"}"#, "shader_instances[0].fingerprint"),
        (r#"{"id": "metal/../x"}"#, "shader_instances[1].id"),
    ] {
        let patch = json::parse(edit).unwrap();
        let bad = edited(&original, |v| {
            let inst = &mut v.get_mut("shader_instances").unwrap().as_array_mut().unwrap()[0];
            for (k, val) in patch.as_object().unwrap() {
                match k.as_str() {
                    "unit" => inst.get_mut("displacement").unwrap().insert(k.clone(), val.clone()),
                    "mode" => inst.get_mut("tessellation").unwrap().insert(k.clone(), val.clone()),
                    _ => inst.insert(k.clone(), val.clone()),
                };
            }
        });
        match from_bytes(&bad) {
            Err(FormatError::Manifest { at: got, .. }) => assert!(got.starts_with(at), "{edit}: error at {got}"),
            other => panic!("{edit}: {other:?}"),
        }
    }
}
