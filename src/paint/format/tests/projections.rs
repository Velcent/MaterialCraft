//! Image fills and fill projections: saved, loaded back unchanged, unknown modes kept (ADR 0007).

use super::corruption::rewrite;
use super::fixtures::noisy;
use super::round_trip::{from_bytes, to_bytes};
use crate::json::{self, Value};
use crate::paint::format::manifest::projection::all_modes;
use crate::paint::{BlendMode, Channel, ContentEffect, Document, EffectKind, Fill, FillSource, ImageFill, Layer, LayerKind, PixelFormat, TextureSet};
use crate::projection::{Filter, ImagePlacement, Projection, ProjectionMode, Repeat, SizeMode};
use crate::zip::ZipReader;
use std::io::Cursor;

fn image_fill() -> Fill {
    let mut images = ImageFill::default();
    images.set_image(Channel::BaseColor, noisy(5, 3, PixelFormat::from_id("rgba8").unwrap(), 7));
    images.set_value(Channel::Roughness, [0.25, 0.25, 0.25, 1.0]);
    let placement = ImagePlacement { tiling: [2.0, -3.0], rotation: 12.5, offset: [0.1, -0.2], size_mode: SizeMode::Custom, physical_size: Some([10.0, 20.0]) };
    let projection = Projection { filter: Filter::Nearest, repeat: Repeat::U, ..Projection::uv(placement) };
    Fill::new(FillSource::Image(images)).with_projection(projection)
}

fn document() -> Document {
    let mut set = TextureSet::new("s", "S", (8, 8)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
    let mut layer = Layer::new("img", "Logo", LayerKind::Fill(image_fill())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0);
    for mode in all_modes() {
        let fill = Fill::uniform(vec![(Channel::Roughness, [0.5; 4])]).with_projection(Projection { mode, ..Projection::default() });
        layer = layer.with_effect(ContentEffect::new(Vec::new(), EffectKind::Fill(fill)).with_blend(Channel::Roughness, BlendMode::Normal, 1.0));
    }
    layer = layer.with_effect(ContentEffect::new(Vec::new(), EffectKind::Fill(image_fill())).with_blend(Channel::BaseColor, BlendMode::Multiply, 0.5));
    set.layers.push(layer);
    let mut doc = Document::default();
    doc.texture_sets.push(set);
    doc
}

fn manifest(bytes: &[u8]) -> Value {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    json::parse(std::str::from_utf8(&zip.read("document.json").unwrap()).unwrap()).unwrap()
}

#[test]
fn image_fills_and_every_mode_round_trip() {
    let doc = document();
    doc.validate().unwrap();
    let bytes = to_bytes(&doc);
    let back = from_bytes(&bytes).unwrap();
    assert_eq!(back, doc);
    assert_eq!(to_bytes(&back), bytes, "load → save reproduces the file");
    let v = manifest(&bytes);
    let layer = &v["texture_sets"][0]["layers"][0];
    assert_eq!(layer["fill"]["images"]["base_color"]["path"].as_str(), Some("textures/s/layers/img/fill-base_color.png"));
    assert_eq!(layer["fill"]["images"]["base_color"]["size"].to_string(), "[5,3]");
    assert_eq!(layer["fill"]["projection"]["mode"].as_str(), Some("uv"));
    assert_eq!(layer["fill"]["projection"]["image"]["tiling"].to_string(), "[2,-3]");
    // the default projection is left out
    assert!(layer["effects"][0]["fill"].get("projection").is_none());
    assert_eq!(layer["effects"][3]["fill"]["projection"]["mode"].as_str(), Some("planar"));
    let last = all_modes().len();
    assert_eq!(
        layer["effects"][last]["fill"]["images"]["base_color"]["path"].as_str(),
        Some(format!("textures/s/layers/img/effect-{last}-fill-base_color.png").as_str())
    );
}

/// Rewrites the first layer's fill projection with `edit`.
fn with_projection(edit: impl Fn(&mut Value)) -> Vec<u8> {
    rewrite(&to_bytes(&document()), |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        let set = &mut v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0];
        let layer = &mut set.get_mut("layers").unwrap().as_array_mut().unwrap()[0];
        edit(layer.get_mut("fill").unwrap().get_mut("projection").unwrap());
        Some(v.to_string_pretty().into_bytes())
    })
}

#[test]
fn unknown_modes_and_values_load_as_not_computable_and_are_kept() {
    for (key, value) in [("mode", "hologram"), ("filter", "lanczos"), ("repeat", "mirror")] {
        let future = with_projection(|p| {
            p.insert(key, value);
            p.insert("focus", 3);
        });
        let doc = from_bytes(&future).unwrap();
        let LayerKind::Fill(f) = &doc.texture_sets[0].layers[0].kind else { panic!("fill layer expected") };
        let ProjectionMode::Unknown(u) = &f.projection.mode else { panic!("{key}: unknown projection expected, got {:?}", f.projection) };
        assert_eq!(u.mode, if key == "mode" { "hologram" } else { "uv" });
        let saved = manifest(&to_bytes(&doc));
        let p = &saved["texture_sets"][0]["layers"][0]["fill"]["projection"];
        assert_eq!(p[key].as_str(), Some(value));
        assert_eq!(p["focus"].as_f64(), Some(3.0));
        assert_eq!(p["image"]["rotation"].as_f64(), Some(12.5));
    }
}

#[test]
fn unknown_fields_inside_a_known_projection_are_kept() {
    let future = with_projection(|p| {
        p.insert("focus", 3);
        p.get_mut("image").unwrap().insert("pivot", vec![0.5, 0.5]);
    });
    let doc = from_bytes(&future).unwrap();
    let LayerKind::Fill(f) = &doc.texture_sets[0].layers[0].kind else { panic!("fill layer expected") };
    assert_eq!(f.projection.mode, ProjectionMode::Uv);
    let saved = manifest(&to_bytes(&doc));
    let p = &saved["texture_sets"][0]["layers"][0]["fill"]["projection"];
    assert_eq!(p["focus"].as_f64(), Some(3.0));
    assert_eq!(p["image"]["pivot"].to_string(), "[0.5,0.5]");
}

#[test]
fn a_channel_with_an_image_and_a_value_is_rejected() {
    let bad = rewrite(&to_bytes(&document()), |name, data| {
        if name != "document.json" {
            return Some(data);
        }
        let mut v = json::parse(std::str::from_utf8(&data).unwrap()).unwrap();
        let set = &mut v.get_mut("texture_sets").unwrap().as_array_mut().unwrap()[0];
        let layer = &mut set.get_mut("layers").unwrap().as_array_mut().unwrap()[0];
        layer.get_mut("fill").unwrap().get_mut("values").unwrap().insert("base_color", vec![1, 1, 1, 1]);
        Some(v.to_string_pretty().into_bytes())
    });
    let err = from_bytes(&bad).unwrap_err().to_string();
    assert!(err.contains("both an image and a value"), "{err}");
}
