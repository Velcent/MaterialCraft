//! Texture fills (format 0.16): saved, loaded back unchanged, malformed ones rejected, and read by an older
//! reader as a uniform fill that keeps the textures as unknown fields.

use super::corruption::rewrite;
use super::round_trip::{from_bytes, to_bytes};
use crate::json::{self, Value};
use crate::paint::effect::Procedural;
use crate::paint::{BlendMode, Channel, ContentEffect, Document, EffectKind, Fill, FillSource, Layer, LayerKind, TextureFill, TextureSet};
use crate::projection::{ImagePlacement, Projection};
use crate::zip::ZipReader;
use std::io::Cursor;

fn texture_fill() -> Fill {
    let original = Procedural::new("substance:ratio_perlin_noise").with_params(vec![("scale".into(), "16".into())]);
    let perlin = Procedural::new("perlin").with_params(vec![("scale".into(), "2".into()), ("seed".into(), "7".into())]);
    let mut f = TextureFill::on(&[Channel::Roughness, Channel::Height], &perlin);
    f.set_texture(Channel::Height, Procedural::new("checker"));
    f.set_value(Channel::BaseColor, [0.25, 0.5, 0.75, 1.0]);
    let projection = Projection::uv(ImagePlacement { tiling: [3.0, 3.0], ..ImagePlacement::default() });
    Fill::new(FillSource::Texture(f)).with_projection(projection).replacing(original)
}

fn document() -> Document {
    let mut set = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness).with_channel(Channel::Height);
    let effect = ContentEffect::new(Vec::new(), EffectKind::Fill(texture_fill())).with_blend(Channel::Roughness, BlendMode::Multiply, 0.5);
    let layer = Layer::new("tex", "Noise", LayerKind::Fill(texture_fill())).with_blend(Channel::Roughness, BlendMode::Normal, 1.0).with_effect(effect);
    set.layers.push(layer);
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    doc
}

fn manifest(bytes: &[u8]) -> Value {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    json::parse(std::str::from_utf8(&zip.read("document.json").unwrap()).unwrap()).unwrap()
}

/// The document with the first layer's fill object changed by `edit`.
fn with_fill(edit: impl Fn(&mut Value)) -> Vec<u8> {
    rewrite(&to_bytes(&document()), |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        let layer = &mut v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0].get_mut("layers").unwrap().as_array_mut().unwrap()[0];
        edit(layer.get_mut("fill").unwrap());
        Some(v.to_string_pretty().into_bytes())
    })
}

#[test]
fn texture_fills_round_trip() {
    let doc = document();
    doc.validate().unwrap();
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back, doc);
    assert_eq!(to_bytes(&back), bytes, "load → save reproduces the file");
    let v = manifest(&bytes);
    assert_eq!(v["format_version"].as_str(), Some("0.16"));
    let fill = &v["texture_sets"][0]["layers"][0]["fill"];
    assert_eq!(fill["textures"]["roughness"]["id"].as_str(), Some("perlin"));
    assert_eq!(fill["textures"]["roughness"]["params"]["scale"].as_str(), Some("2"));
    assert_eq!(fill["textures"]["height"]["id"].as_str(), Some("checker"));
    assert_eq!(fill["values"]["base_color"].to_string(), "[0.25,0.5,0.75,1]");
    assert_eq!(fill["original"]["id"].as_str(), Some("substance:ratio_perlin_noise"));
    assert_eq!(fill["projection"]["mode"].as_str(), Some("uv"));
}

#[test]
fn malformed_texture_fills_are_rejected() {
    let both = with_fill(|f| {
        f.insert("images", Value::object());
    });
    assert!(from_bytes(&both).is_err(), "images and textures");
    let twice = with_fill(|f| {
        let mut values = f.get("values").unwrap().clone();
        values.insert("height", Value::Array(vec![0.5.into(), 0.5.into(), 0.5.into(), 1.0.into()]));
        f.insert("values", values);
    });
    assert!(from_bytes(&twice).is_err(), "a texture and a value for one channel");
    let no_id = with_fill(|f| {
        let mut textures = f.get("textures").unwrap().clone();
        textures.insert("roughness", Value::object());
        f.insert("textures", textures);
    });
    assert!(from_bytes(&no_id).is_err(), "a texture without an id");
}

#[test]
fn a_fill_without_textures_stays_uniform() {
    // what a 0.15 file holds, and what a 0.15 reader makes of a texture fill (keeping "textures" as an unknown
    // field): the values alone
    let plain = with_fill(|f| {
        if let Value::Object(entries) = f {
            entries.retain(|(k, _)| k != "textures");
        }
    });
    let doc = from_bytes(&plain).unwrap();
    let LayerKind::Fill(f) = &doc.texture_sets[0].layers[0].kind else { panic!("a fill layer") };
    assert_eq!(f.source, FillSource::Uniform(vec![(Channel::BaseColor, [0.25, 0.5, 0.75, 1.0])]));
}
