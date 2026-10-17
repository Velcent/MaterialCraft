//! [`MaskEvaluator`]: a Painter mask (`maskActions` from `maskInitial`) as a painted [`Mask`], where it is
//! made only of uniform and bitmap fills (and empty paint groups). Generators, filters, strokes, levels and
//! references are not computable yet; the layer then gets no mask and the reason is reported.

use super::blend_table::BlendTable;
use super::blending::ResolvedBlend;
use super::computability::Computability;
use super::images::{ImageSource, fit, grey_values};
use crate::model::{MaskInitial, PainterEffect, PainterMask, PainterSource};
use materialcraft::paint::{Mask, PixelFormat, PixelLayout, SampleType, TiledImage};

/// Stored format of imported masks (16 bits: constant masks stay exact to well below 8-bit rounding).
const MASK_FORMAT: PixelFormat = PixelFormat::new(PixelLayout::Gray, SampleType::U16);

/// The mask for a layer (`None` = no mask, the layer applies everywhere) and how exact it is.
pub struct MaskOutcome {
    pub mask: Option<Mask>,
    pub status: Computability,
}

/// Grey mask values while evaluating.
enum Grey {
    Constant(f32),
    Pixels(Vec<f32>),
}

pub struct MaskEvaluator<'a> {
    pub table: &'a BlendTable,
    pub images: &'a dyn ImageSource,
    pub resolution: (u32, u32),
}

impl MaskEvaluator<'_> {
    pub fn evaluate(&self, mask: Option<&PainterMask>) -> MaskOutcome {
        let Some(mask) = mask.filter(|m| m.enabled) else {
            return MaskOutcome {
                mask: None,
                status: Computability::Exact,
            };
        };
        let mut grey = match mask.initial {
            MaskInitial::Black => Grey::Constant(0.0),
            MaskInitial::White => Grey::Constant(1.0),
            MaskInitial::Other(n) => return not_computable(format!("mask initial value {n}")),
        };
        let mut status = Computability::Exact;
        self.walk(&mask.effects, &mut grey, &mut status);
        if !status.is_computable() {
            return MaskOutcome { mask: None, status };
        }
        MaskOutcome {
            mask: self.image(grey),
            status,
        }
    }

    fn walk(&self, effects: &[PainterEffect], grey: &mut Grey, status: &mut Computability) {
        for effect in effects.iter().filter(|e| e.is_enabled()) {
            let stop = |status: &mut Computability, why: String| status.and(&Computability::not_computable(why));
            match effect {
                PainterEffect::Group(g) => self.walk(&g.effects, grey, status),
                PainterEffect::Paint(p) if p.strokes > 0 => stop(status, "painted mask (strokes)".into()),
                PainterEffect::Paint(_) | PainterEffect::Anchor(_) => {}
                PainterEffect::Fill(fill) => {
                    let blend = ResolvedBlend::of_mask(&fill.header.blending, self.table);
                    if !blend.contributes() {
                        continue;
                    }
                    let top = match fill.sources.first() {
                        None => continue,
                        Some(PainterSource::Uniform(u)) => Grey::Constant(u.color[0]),
                        Some(PainterSource::Bitmap(b)) => match self.images.load(&b.url) {
                            Ok(loaded) => {
                                let (image, note) = fit(loaded.image, self.resolution);
                                status.and(&note);
                                Grey::Pixels(grey_values(&image))
                            }
                            Err(e) => return stop(status, format!("mask {e}")),
                        },
                        Some(PainterSource::Procedural(p)) => {
                            let name = p.procedural.as_ref().map_or("?", |p| p.name());
                            return stop(status, format!("mask generator \"{name}\""));
                        }
                        Some(PainterSource::Reference(_)) => return stop(status, "mask reads an anchor".into()),
                        Some(PainterSource::Other(raw)) => return stop(status, format!("mask source {}", raw.class)),
                    };
                    status.and(&blend.status());
                    blend_into(grey, &top, &blend);
                }
                PainterEffect::Filter(f) => {
                    let name = f.procedural.as_ref().map_or("?", |p| p.name());
                    stop(status, format!("mask filter \"{name}\""));
                }
                PainterEffect::Levels(_) => stop(status, "mask levels".into()),
                PainterEffect::ColorSelection(_) => stop(status, "mask colour selection".into()),
                PainterEffect::PolygonFill(_) => stop(status, "mask polygon fill".into()),
                PainterEffect::Other(raw) => stop(status, format!("mask effect {}", raw.class)),
            }
            if !status.is_computable() {
                return;
            }
        }
    }

    /// `None` for an all-white mask; an unwritten image (reads as zero) for an all-black one.
    fn image(&self, grey: Grey) -> Option<Mask> {
        let (w, h) = self.resolution;
        let mut image = TiledImage::new(w, h, MASK_FORMAT).ok()?;
        match grey {
            Grey::Constant(c) if c >= 1.0 => return None,
            Grey::Constant(c) if c <= 0.0 => {}
            Grey::Constant(c) => image.fill([c, c, c, 1.0]),
            Grey::Pixels(values) => {
                let all = image.all_tiles();
                image.update_tiles(&all, |r, px| {
                    for (i, p) in px.iter_mut().enumerate() {
                        let (x, y) = (r.x as usize + i % r.w as usize, r.y as usize + i / r.w as usize);
                        let v = values.get(y * w as usize + x).copied().unwrap_or(0.0);
                        *p = [v, v, v, 1.0];
                    }
                });
            }
        }
        Some(Mask::from_image(image))
    }
}

fn not_computable(why: String) -> MaskOutcome {
    MaskOutcome {
        mask: None,
        status: Computability::not_computable(why),
    }
}

/// Blends `top` onto `grey` with the effect's mode and opacity, clamped to 0..1.
fn blend_into(grey: &mut Grey, top: &Grey, blend: &ResolvedBlend) {
    let mix = |b: f32, t: f32| {
        let v = blend.mode().mix([b, b, b, 1.0], [t, t, t, 1.0], blend.opacity)[0];
        if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }
    };
    *grey = match (&*grey, top) {
        (Grey::Constant(b), Grey::Constant(t)) => Grey::Constant(mix(*b, *t)),
        (Grey::Constant(b), Grey::Pixels(t)) => Grey::Pixels(t.iter().map(|t| mix(*b, *t)).collect()),
        (Grey::Pixels(b), Grey::Constant(t)) => Grey::Pixels(b.iter().map(|b| mix(*b, *t)).collect()),
        (Grey::Pixels(b), Grey::Pixels(t)) => Grey::Pixels(b.iter().zip(t).map(|(b, t)| mix(*b, *t)).collect()),
    };
}
