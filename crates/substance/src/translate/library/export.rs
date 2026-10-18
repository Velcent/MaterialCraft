//! [`LibraryExport`]: the candidates the user ticked, made into MaterialCraft assets ([`NewAsset`]) and written into
//! their library folder by the assets module. Images are copied as stored (PNG for 8/16-bit images, OpenEXR for float
//! ones such as environment maps); alphas as 16-bit grey PNGs of the coverage the replay's tips use (grey × alpha);
//! brushes as `.mcbrush` presets with a stroke thumbnail, written after the alphas they bring so their tips name the
//! written files. Every asset records its project and origin ([`Provenance::Imported`]); vendor content is tagged
//! `not-shareable`. The imported document is not touched.

use super::candidate::{CandidateKind, CandidateSource, LibraryCandidate, LibraryCandidates};
use crate::translate::layers::images::{ImageSource, grey_values};
use crate::translate::layers::replay::tip::bitmap_tip;
use materialcraft::assets::{NewAsset, Provenance, SavedAsset};
use materialcraft::deflate::Level;
use materialcraft::paint::brush::{Tip, file};
use materialcraft::paint::format::images::{Encoding, encode};
use materialcraft::png::{ColorType, encode_raw};
use materialcraft_brush::{TipAlpha, stroke_preview};
use std::collections::HashMap;
use std::path::Path;

/// Side of a brush's stroke thumbnail.
const THUMB_PX: u32 = 128;
/// Tag of every imported asset, and of the vendor's.
pub const IMPORTED_TAG: &str = "imported";
pub const NOT_SHAREABLE_TAG: &str = "not-shareable";

/// What happened to one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    pub key: String,
    pub name: String,
    pub result: Result<SavedAsset, String>,
}

/// Writes ticked candidates of one project.
pub struct LibraryExport<'a> {
    candidates: &'a LibraryCandidates,
    images: &'a dyn ImageSource,
}

impl<'a> LibraryExport<'a> {
    /// `images` reads the project's stored images (the importer's [`ImageSource`]).
    pub fn new(candidates: &'a LibraryCandidates, images: &'a dyn ImageSource) -> LibraryExport<'a> {
        LibraryExport { candidates, images }
    }

    /// Writes the candidates `keys` names (and the alphas their brushes bring) into `library`. Nothing is written
    /// for an empty `keys`. One entry per written candidate; a failure stops nothing else.
    pub fn write(&self, keys: &[String], library: &Path) -> Vec<Written> {
        let mut alphas: HashMap<String, (String, Option<TipAlpha>)> = HashMap::new();
        let mut out = Vec::new();
        for c in self.candidates.closure(keys) {
            let made = self.make(c, &alphas);
            let result = made.and_then(|(asset, tip)| {
                let saved = asset.save(library).map_err(|e| e.to_string())?;
                if c.kind == CandidateKind::Alpha {
                    alphas.insert(c.key.clone(), (saved.reference.clone(), tip));
                }
                Ok(saved)
            });
            out.push(Written {
                key: c.key.clone(),
                name: c.name.clone(),
                result,
            });
        }
        out
    }

    /// The asset of `c` (and for an alpha its tip, for the brushes that bring it).
    fn make(
        &self,
        c: &LibraryCandidate,
        alphas: &HashMap<String, (String, Option<TipAlpha>)>,
    ) -> Result<(NewAsset, Option<TipAlpha>), String> {
        let mut tip = None;
        let (extension, bytes, thumbnail) = match &c.source {
            CandidateSource::Image { url } => {
                let loaded = self.images.load(url)?;
                match c.kind {
                    CandidateKind::Alpha => {
                        let (w, h) = loaded.image.size();
                        tip = bitmap_tip(&loaded.image);
                        ("png".to_string(), grey16(w, h, &grey_values(&loaded.image)), None)
                    }
                    _ => {
                        let encoding = Encoding::for_format(loaded.image.format());
                        (encoding.extension().to_string(), encode(&loaded.image)?, None)
                    }
                }
            }
            CandidateSource::Brush { preset, alpha } => {
                let mut preset = preset.clone();
                preset.name = c.name.clone();
                let mut tip_alpha = None;
                if let Some(key) = alpha {
                    let (reference, t) = alphas.get(key).ok_or("its alpha was not written")?;
                    preset.tip = Tip::Alpha(reference.clone());
                    tip_alpha = t.clone();
                }
                let picture = stroke_preview(&preset, tip_alpha, THUMB_PX, THUMB_PX);
                let thumb = grey8(THUMB_PX as usize, &picture);
                (
                    file::EXTENSION.to_string(),
                    preset.to_file_text().into_bytes(),
                    Some(thumb),
                )
            }
        };
        let asset = NewAsset {
            usage: c.kind.usage(),
            name: c.name.clone(),
            extension,
            bytes,
            thumbnail,
            tags: tags(c),
            description: Some(description(&self.candidates.project, c)),
            provenance: Provenance::Imported {
                project: self.candidates.project.clone(),
                origin: c.origin.describe(),
                shareable: c.shareable(),
            },
        };
        Ok((asset, tip))
    }
}

fn tags(c: &LibraryCandidate) -> Vec<String> {
    let mut tags = vec![IMPORTED_TAG.to_string(), "painter".to_string()];
    if c.kind == CandidateKind::Stencil {
        tags.push(c.kind.label().to_string());
    }
    if !c.shareable() {
        tags.push(NOT_SHAREABLE_TAG.to_string());
    }
    tags
}

fn description(project: &str, c: &LibraryCandidate) -> String {
    let file = project.rsplit(['/', '\\']).next().unwrap_or(project);
    let mut d = format!("Imported from {file} ({}).", c.origin.describe());
    if !c.shareable() {
        d.push_str(" From this project, not shareable.");
    }
    d
}

/// A 16-bit grey PNG of `values` (0..1).
fn grey16(w: u32, h: u32, values: &[f32]) -> Vec<u8> {
    let samples: Vec<u8> = values
        .iter()
        .flat_map(|v| ((clean(*v) * 65535.0).round() as u16).to_be_bytes())
        .collect();
    encode_raw(w as usize, h as usize, ColorType::Gray, 16, &samples, Level::Default)
}

/// An 8-bit grey PNG, `side` square, of `values` (0..1).
fn grey8(side: usize, values: &[f32]) -> Vec<u8> {
    let samples: Vec<u8> = values.iter().map(|v| (clean(*v) * 255.0).round() as u8).collect();
    encode_raw(side, side, ColorType::Gray, 8, &samples, Level::Default)
}

fn clean(v: f32) -> f32 {
    if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
}
