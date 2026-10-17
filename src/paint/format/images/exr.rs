//! OpenEXR entries: HALF or FLOAT channels `Y`, `A`, `R`, `G`, `B`, ZIP compression.

use crate::exr::{self, Compression, ExrChannel, ExrImage, SampleKind};
use crate::half::{f16_to_f32, f32_to_f16};
use crate::paint::{PixelFormat, PixelLayout, SampleType, TiledImage};

/// EXR channel names of a layout, in sample order.
fn exr_names(layout: PixelLayout) -> &'static [&'static str] {
    match layout {
        PixelLayout::Gray => &["Y"],
        PixelLayout::GrayAlpha => &["Y", "A"],
        PixelLayout::Rgb => &["R", "G", "B"],
        PixelLayout::Rgba => &["R", "G", "B", "A"],
    }
}

fn exr_kind(sample: SampleType) -> SampleKind {
    if sample == SampleType::F32 { SampleKind::Float } else { SampleKind::Half }
}

pub(super) fn encode(img: &TiledImage) -> Result<Vec<u8>, String> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let f = img.format();
    let raw = img.to_raw();
    let (n, size) = (f.layout.components(), f.sample.bytes());
    let channels = exr_names(f.layout)
        .iter()
        .enumerate()
        .map(|(c, name)| {
            let data = raw.chunks_exact(n * size).map(|px| read_float(f.sample, &px[c * size..])).collect();
            ExrChannel::new(name, exr_kind(f.sample), data)
        })
        .collect();
    drop(raw);
    exr::encode_with(&ExrImage { width: w, height: h, channels }, Compression::Zip)
}

pub(super) fn decode(bytes: &[u8], format: PixelFormat, size: (u32, u32)) -> Result<Vec<u8>, String> {
    let found = exr_size(bytes).ok_or("not a readable OpenEXR header")?;
    if found != (size.0 as u64, size.1 as u64) {
        return Err(format!("EXR is {}x{}, expected {}x{}", found.0, found.1, size.0, size.1));
    }
    let img = exr::decode(bytes)?;
    let names = exr_names(format.layout);
    let kind = exr_kind(format.sample);
    let mut channels = Vec::with_capacity(names.len());
    for name in names {
        let ch = img.channel(name).ok_or_else(|| format!("EXR lacks channel {name}"))?;
        if ch.sample != kind {
            return Err(format!("EXR channel {name} is {:?}, expected {kind:?}", ch.sample));
        }
        channels.push(&ch.data);
    }
    if img.channels.len() != names.len() {
        return Err(format!("EXR has {} channels, expected {}", img.channels.len(), names.len()));
    }
    let size = format.sample.bytes();
    let mut raw = vec![0u8; img.width * img.height * format.bytes_per_pixel()];
    for (i, px) in raw.chunks_exact_mut(format.bytes_per_pixel()).enumerate() {
        for (c, data) in channels.iter().enumerate() {
            write_float(format.sample, data[i], &mut px[c * size..]);
        }
    }
    Ok(raw)
}

fn read_float(sample: SampleType, b: &[u8]) -> f32 {
    match sample {
        SampleType::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
        _ => f16_to_f32(u16::from_le_bytes([b[0], b[1]])),
    }
}

fn write_float(sample: SampleType, v: f32, b: &mut [u8]) {
    match sample {
        SampleType::F32 => b[..4].copy_from_slice(&v.to_le_bytes()),
        _ => b[..2].copy_from_slice(&f32_to_f16(v).to_le_bytes()),
    }
}

/// Width and height from the `dataWindow` attribute of an OpenEXR header, without decoding.
fn exr_size(b: &[u8]) -> Option<(u64, u64)> {
    let mut pos = 8;
    let cstr = |pos: &mut usize| -> Option<&[u8]> {
        let len = b.get(*pos..)?.iter().position(|&c| c == 0)?;
        let s = &b[*pos..*pos + len];
        *pos += len + 1;
        Some(s)
    };
    loop {
        let name = cstr(&mut pos)?;
        if name.is_empty() {
            return None;
        }
        let kind = cstr(&mut pos)?;
        let len = u32::from_le_bytes(b.get(pos..pos + 4)?.try_into().ok()?) as usize;
        let value = b.get(pos + 4..(pos + 4).checked_add(len)?)?;
        pos += 4 + len;
        if name == b"dataWindow" && kind == b"box2i" && len == 16 {
            let v: Vec<i64> = value.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as i64).collect();
            return Some(((v[2] - v[0] + 1).max(0) as u64, (v[3] - v[1] + 1).max(0) as u64));
        }
    }
}
