//! Reading: header, line-offset table, then every chunk, each placed by the y coordinate it carries.

use super::header::Header;
use super::{ExrChannel, ExrImage, SampleKind};
use crate::half::f16_to_f32;

/// Decode a single-part scanline OpenEXR file (NONE, RLE, ZIPS or ZIP compression). Channels come back sorted by
/// name; the data window becomes the image (its origin is dropped).
pub fn decode(b: &[u8]) -> Result<ExrImage, String> {
    let (header, table_at) = Header::read(b)?;
    let (w, h) = (header.width(), header.height());
    let lines = header.compression.lines_per_block();
    let blocks = h.div_ceil(lines);

    // Refuse sizes the file cannot possibly hold before allocating anything.
    let raw_total = header.line_bytes().checked_mul(h).ok_or("image too large")?;
    if raw_total / header.compression.max_expansion() > b.len() {
        return Err("image larger than the file can hold".into());
    }
    let table = b.get(table_at..table_at + blocks.checked_mul(8).ok_or("image too large")?).ok_or("truncated line offset table")?;

    let mut channels: Vec<ExrChannel> = header.channels.iter().map(|(name, kind)| ExrChannel::new(name, *kind, vec![0.0; w * h])).collect();
    let mut seen = vec![false; blocks];
    for entry in table.chunks_exact(8) {
        let offset = u64::from_le_bytes([entry[0], entry[1], entry[2], entry[3], entry[4], entry[5], entry[6], entry[7]]);
        let at = usize::try_from(offset).map_err(|_| "chunk offset out of range")?;
        let head = b.get(at..at.checked_add(8).ok_or("chunk offset out of range")?).ok_or("chunk offset out of range")?;
        let y = i32::from_le_bytes([head[0], head[1], head[2], head[3]]) as i64;
        let size = usize::try_from(i32::from_le_bytes([head[4], head[5], head[6], head[7]])).map_err(|_| "negative chunk size")?;
        let data = b.get(at + 8..at + 8 + size).ok_or("truncated chunk")?;

        let rel = y - header.data_window[1] as i64;
        if rel < 0 || rel as usize >= h || !(rel as usize).is_multiple_of(lines) {
            return Err(format!("chunk has invalid y {y}"));
        }
        let block = rel as usize / lines;
        if std::mem::replace(&mut seen[block], true) {
            return Err(format!("duplicate chunk for y {y}"));
        }
        let y0 = rel as usize;
        let n_lines = lines.min(h - y0);
        let raw = header.compression.decompress(data, n_lines * header.line_bytes())?;
        let mut pos = 0;
        for line in y0..y0 + n_lines {
            for ch in channels.iter_mut() {
                let bytes = w * ch.sample.size();
                read_samples(&raw[pos..pos + bytes], ch.sample, &mut ch.data[line * w..(line + 1) * w]);
                pos += bytes;
            }
        }
    }
    Ok(ExrImage { width: w, height: h, channels })
}

fn read_samples(src: &[u8], kind: SampleKind, dst: &mut [f32]) {
    match kind {
        SampleKind::Half => src.chunks_exact(2).zip(dst).for_each(|(s, d)| *d = f16_to_f32(u16::from_le_bytes([s[0], s[1]]))),
        SampleKind::Float => src.chunks_exact(4).zip(dst).for_each(|(s, d)| *d = f32::from_le_bytes([s[0], s[1], s[2], s[3]])),
        SampleKind::Uint => src.chunks_exact(4).zip(dst).for_each(|(s, d)| *d = u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32),
    }
}
