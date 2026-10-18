//! Reference materials: one uniform material per entry of physicallybased.info's material values (CC0), vendored in
//! `content/physicallybased/materials.json` (values only; see the README there). Each is a small graph with the
//! "Metallic Roughness" outputs: the base colour (the linear sRGB value, sRGB-encoded as base colour textures are),
//! metalness and roughness. MaterialCraft's material has no specular level, so the index of refraction stays in the
//! description, with the reflectance it gives. The thumbnail is the tray's material ball, rendered here; the site's
//! own preview images (CC BY) are never used.

use crate::build::Options;
use crate::image::{THUMB_PX, rgb8};
use crate::recipe::sidecar;
use crate::{Made, Recipe, StarterError};
use materialcraft::assets::{CC0, Provenance, Usage};
use materialcraft::graph::Graph;
use materialcraft::json::{self, Value};
use materialcraft::material_fill::{self, Material};

/// The vendored values.
const DATA: &str = include_str!("../content/physicallybased/materials.json");

/// Where the values come from, as the provenance records it.
const SOURCE_URL: &str = "https://github.com/AntonPalmqvist/physically-based-api/blob/main/deploy/v2/materials.json";
const AUTHOR: &str = "Anton Palmqvist and the physicallybased.info contributors";

/// The library folder of the reference materials, below the materials folder.
const FOLDER: &str = "reference";

/// Size a uniform material is evaluated at for its thumbnail.
const THUMB_EVAL: usize = 16;

pub fn recipes() -> Vec<Box<dyn Recipe>> {
    match entries() {
        Ok(all) => all.into_iter().map(|e| Box::new(e) as Box<dyn Recipe>).collect(),
        Err(reason) => vec![Box::new(Unreadable(reason))],
    }
}

/// One entry's values.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    id: String,
    name: String,
    category: String,
    /// Linear sRGB.
    color: [f32; 3],
    metalness: f32,
    roughness: f32,
    ior: Option<f32>,
    transmissive: bool,
    tags: Vec<String>,
}

/// Every entry of the vendored file, in its order.
fn entries() -> Result<Vec<Entry>, String> {
    let v = json::parse(DATA).map_err(|e| format!("the vendored physicallybased.info values don't parse: {e:?}"))?;
    let data = v.get("data").and_then(Value::as_array).ok_or("the vendored values have no data list")?;
    data.iter().map(entry).collect()
}

fn entry(m: &Value) -> Result<Entry, String> {
    let name = m.get("name").and_then(Value::as_str).ok_or("an entry has no name")?.to_owned();
    let number = |key: &str| m.get(key).and_then(Value::as_f64).map(|v| v as f32).filter(|v| v.is_finite());
    let color = m
        .get("color")
        .and_then(Value::as_array)
        .and_then(|cs| cs.iter().find(|c| c.get("colorSpace").and_then(Value::as_str) == Some("srgb-linear")))
        .and_then(|c| c.get("color"))
        .and_then(Value::as_array)
        .and_then(|c| match c.as_slice() {
            [r, g, b] => Some([r.as_f64()? as f32, g.as_f64()? as f32, b.as_f64()? as f32]),
            _ => None,
        })
        .ok_or_else(|| format!("{name} has no linear sRGB colour"))?;
    let strings = |key: &str| -> Vec<String> {
        m.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_lowercase).collect()).unwrap_or_default()
    };
    let category = strings("category").into_iter().next().unwrap_or_else(|| "other".into());
    let mut tags = strings("tags");
    tags.extend(m.get("group").and_then(Value::as_str).map(str::to_lowercase));
    Ok(Entry {
        id: slug(&name),
        category,
        color,
        metalness: number("metalness").ok_or_else(|| format!("{name} has no metalness"))?.clamp(0.0, 1.0),
        roughness: number("roughness").ok_or_else(|| format!("{name} has no roughness"))?.clamp(0.0, 1.0),
        ior: number("ior"),
        transmissive: number("transmission").is_some_and(|t| t > 0.0),
        tags,
        name,
    })
}

/// A file id from a name: lower case, runs of other characters as one `-` ("Vodka (40%)" → "vodka-40").
fn slug(name: &str) -> String {
    let mut s = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.is_empty() && !s.ends_with('-') {
            s.push('-');
        }
    }
    s.trim_end_matches('-').to_owned()
}

/// A linear value sRGB-encoded.
fn encode(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

impl Entry {
    /// The material's graph: uniform values on the "Metallic Roughness" outputs.
    fn graph(&self) -> String {
        let [r, g, b] = self.color.map(encode);
        let (m, ro) = (self.metalness, self.roughness);
        format!(
            "# {name}: a reference material from physicallybased.info's values (CC0), built by mcstarter\n\
             size 256\n\
             node 1 color r={r:.4} g={g:.4} b={b:.4}\n\
             node 2 color r={ro:.4} g={ro:.4} b={ro:.4}\n\
             node 3 color r={m:.4} g={m:.4} b={m:.4}\n\
             node 90 color r=0.5 g=0.5 b=1\n\
             node 91 color r=1 g=1 b=1\n\
             node 92 color r=0.5 g=0.5 b=0.5\n\
             output basecolor 1\n\
             output normal 90\n\
             output roughness 2\n\
             output metallic 3\n\
             output height 92\n\
             output ambient_occlusion 91\n",
            name = self.name
        )
    }

    /// The description, in our own words, from the values.
    fn description(&self) -> String {
        let [r, g, b] = self.color;
        let kind = if self.metalness >= 0.5 { "a metal" } else { "a non-metal" };
        let mut d = format!(
            "{}: measured reference values from physicallybased.info ({} category), as a plain uniform material. Base colour {r:.3}, {g:.3}, {b:.3} \
             (linear sRGB); {kind}; roughness {:.2}, a typical value rather than a measurement.",
            self.name, self.category, self.roughness
        );
        if let Some(ior) = self.ior {
            let f0 = ((ior - 1.0) / (ior + 1.0)).powi(2);
            d.push_str(&format!(
                " Index of refraction {ior:.3}: {:.1} % reflectance head-on (specular level {:.2}); MaterialCraft's material has no specular input, so it is not applied.",
                f0 * 100.0,
                (f0 / 0.08).min(1.0)
            ));
        }
        if self.transmissive {
            d.push_str(" The real material lets light through; this one is opaque.");
        }
        d
    }

    fn tags(&self) -> Vec<&str> {
        let mut tags = vec!["reference", "physicallybased", self.category.as_str(), if self.metalness >= 0.5 { "metal" } else { "dielectric" }];
        for t in &self.tags {
            if !tags.contains(&t.as_str()) {
                tags.push(t);
            }
        }
        tags
    }
}

impl Recipe for Entry {
    fn path(&self) -> String {
        format!("{}/{FOLDER}/{}.mca", Usage::Material.folder(), self.id)
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        let fail = |e: &dyn std::fmt::Display| StarterError::recipe(&self.path(), e);
        let mut graph = Graph::parse(&self.graph()).map_err(|e| fail(&e))?;
        graph.meta.insert("identifier".into(), format!("{FOLDER}-{}", self.id));
        graph.meta.insert("label".into(), self.name.clone());
        let text = graph.publish().map_err(|e| fail(&e))?;
        let material = Material::from_text(&text, &[]).map_err(|e| fail(&e))?;
        let ball = material_fill::ball(&material, THUMB_EVAL, THUMB_PX).map_err(|e| fail(&e))?;
        let provenance = Provenance::Cc0 { url: SOURCE_URL.into(), licence: CC0.into(), author: Some(AUTHOR.into()) };
        Ok(Made {
            bytes: text.into_bytes(),
            sidecar: sidecar(&self.name, Usage::Material, &self.tags(), &self.description(), provenance),
            thumbnail: Some(rgb8(&ball)),
        })
    }
}

/// Stands in for the reference materials when the vendored file can't be read: the build fails with the reason.
struct Unreadable(String);

impl Recipe for Unreadable {
    fn path(&self) -> String {
        format!("{}/{FOLDER}", Usage::Material.folder())
    }

    fn make(&self, _: &Options) -> Result<Made, StarterError> {
        Err(StarterError::recipe(&self.path(), &self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_reads_with_a_unique_id() {
        let all = entries().unwrap();
        assert!(all.len() >= 100, "{}", all.len());
        for e in &all {
            assert!(!e.id.is_empty(), "{}", e.name);
            assert_eq!(all.iter().filter(|o| o.id == e.id).count(), 1, "{}", e.id);
            assert!(Graph::parse(&e.graph()).is_ok(), "{}", e.id);
        }
        assert_eq!(slug("Vodka (40%)"), "vodka-40");
        assert_eq!(slug("Aluminum (Anodized Red)"), "aluminum-anodized-red");
    }

    #[test]
    fn colours_are_srgb_encoded() {
        assert!((encode(0.214) - 0.5).abs() < 0.01);
        assert_eq!(encode(0.0), 0.0);
        assert!((encode(1.0) - 1.0).abs() < 1e-6);
    }
}
