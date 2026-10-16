//! `thumbnail.png`: a small 8-bit sRGB preview for file browsers. Never read back.

use crate::deflate::Level;
use crate::paint::{Channel, ColorSpace, Compositor, Document, Rect};
use crate::png::{self, ColorType};
use crate::tex::Tex;

/// Longest side of a generated thumbnail.
pub const MAX_SIZE: u32 = 256;

/// Which thumbnail [`super::save_with`] stores.
#[derive(Debug, Clone, Default)]
pub enum Thumbnail {
    /// No `thumbnail.png`.
    None,
    /// The composited base colour of the first texture set that has one, at most 256 pixels on
    /// its longest side; none if no set has a base colour channel.
    #[default]
    FromDocument,
    /// This image (sRGB-encoded RGBA, stored as 8-bit RGBA).
    Image(Tex),
}

impl Thumbnail {
    /// The PNG to store, if any.
    pub fn render(&self, doc: &Document) -> Option<Vec<u8>> {
        match self {
            Thumbnail::None => None,
            Thumbnail::Image(t) if t.w == 0 || t.h == 0 => None,
            Thumbnail::Image(t) => {
                let samples: Vec<u8> = t.px.iter().flatten().map(|&v| to_u8(v)).collect();
                Some(png::encode_raw(
                    t.w,
                    t.h,
                    ColorType::Rgba,
                    8,
                    &samples,
                    Level::Default,
                ))
            }
            Thumbnail::FromDocument => from_document(doc),
        }
    }
}

fn from_document(doc: &Document) -> Option<Vec<u8>> {
    let set = doc
        .texture_sets
        .iter()
        .find(|s| s.has_channel(Channel::BaseColor))?;
    let linear = set.channel_spec(Channel::BaseColor)?.color_space == ColorSpace::Linear;
    let (w, h) = set.resolution;
    let scale = (w.max(h) as f32 / MAX_SIZE as f32).max(1.0);
    let (tw, th) = (
        ((w as f32 / scale) as u32).max(1),
        ((h as f32 / scale) as u32).max(1),
    );
    let compositor = Compositor::new(set);
    let mut row = vec![[0.0f32; 4]; w as usize];
    let mut samples = Vec::with_capacity((tw * th * 3) as usize);
    for ty in 0..th {
        let y = ((ty as f32 + 0.5) * h as f32 / th as f32) as u32;
        let r = Rect {
            x: 0,
            y: y.min(h - 1),
            w,
            h: 1,
        };
        compositor.composite_rect(Channel::BaseColor, r, &mut row);
        for tx in 0..tw {
            let x = (((tx as f32 + 0.5) * w as f32 / tw as f32) as usize).min(w as usize - 1);
            for &c in &row[x][..3] {
                samples.push(to_u8(if linear { linear_to_srgb(c) } else { c }));
            }
        }
    }
    Some(png::encode_raw(
        tw as usize,
        th as usize,
        ColorType::Rgb,
        8,
        &samples,
        Level::Default,
    ))
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
