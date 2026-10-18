// SPDX-License-Identifier: MIT OR Apache-2.0
//! Radiance `.hdr` (RGBE) reader, written from the format's public description: a text header (`#?RADIANCE` or
//! `#?RGBE`, `FORMAT=32-bit_rle_rgbe`, an empty line), a resolution line (`-Y height +X width`, or `+Y` for
//! bottom-up files) and the scanlines, either flat 4-byte pixels, the old run-length scheme (a `1 1 1 n` pixel
//! repeats the previous one) or the newer per-component run-length scanlines (`2 2 hi lo`, then each of the four
//! components as runs). `XYZE` files are converted to linear sRGB primaries. The header's `EXPOSURE` is ignored: the
//! pixels are used as stored ("as authored"). Never panics on damaged input: it returns an error instead.

use crate::tex::Tex;

/// Reads a Radiance file into linear RGB (alpha 1), top row first.
pub fn decode(bytes: &[u8]) -> Result<Tex, String> {
    let mut r = Reader { b: bytes, at: 0 };
    let header = read_header(&mut r)?;
    let (w, h, bottom_up) = read_resolution(&mut r)?;
    let mut px = vec![[0.0f32, 0.0, 0.0, 1.0]; w * h];
    let mut line = vec![[0u8; 4]; w];
    for y in 0..h {
        read_scanline(&mut r, &mut line)?;
        let row = if bottom_up { h - 1 - y } else { y };
        let Some(out) = px.get_mut(row * w..(row + 1) * w) else { return Err("internal row error".into()) };
        for (o, rgbe) in out.iter_mut().zip(&line) {
            let c = rgbe_to_float(*rgbe);
            let c = if header.xyz { xyz_to_srgb(c) } else { c };
            *o = [c[0], c[1], c[2], 1.0];
        }
    }
    Ok(Tex { w, h, px })
}

/// True if `bytes` start like a Radiance file.
pub fn sniff(bytes: &[u8]) -> bool {
    bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE")
}

struct Header {
    xyz: bool,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8, String> {
        let v = self.b.get(self.at).copied().ok_or_else(|| "the file ends inside the pixels".to_string())?;
        self.at += 1;
        Ok(v)
    }

    fn bytes4(&mut self) -> Result<[u8; 4], String> {
        Ok([self.byte()?, self.byte()?, self.byte()?, self.byte()?])
    }

    /// One header line without its newline (at most 4 KiB, so a binary file is refused quickly).
    fn line(&mut self) -> Result<&str, String> {
        let rest = self.b.get(self.at..).unwrap_or(&[]);
        let end = rest.iter().take(4096).position(|&c| c == b'\n').ok_or_else(|| "the header is not a Radiance header".to_string())?;
        let line = rest.get(..end).unwrap_or(&[]);
        self.at += end + 1;
        std::str::from_utf8(line).map(|s| s.trim_end_matches('\r')).map_err(|_| "the header is not text".to_string())
    }
}

fn read_header(r: &mut Reader) -> Result<Header, String> {
    let first = r.line()?;
    if !first.starts_with("#?") {
        return Err("not a Radiance .hdr file (it doesn't start with #?RADIANCE)".into());
    }
    let mut format = None;
    for _ in 0..1000 {
        let line = r.line()?;
        if line.is_empty() {
            return match format.as_deref() {
                None | Some("32-bit_rle_rgbe") => Ok(Header { xyz: false }),
                Some("32-bit_rle_xyze") => Ok(Header { xyz: true }),
                Some(other) => Err(format!("unsupported Radiance pixel format {other:?}")),
            };
        }
        if let Some(f) = line.strip_prefix("FORMAT=") {
            format = Some(f.trim().to_string());
        }
    }
    Err("the Radiance header has no end".into())
}

/// Width, height and whether the first scanline is the bottom row.
fn read_resolution(r: &mut Reader) -> Result<(usize, usize, bool), String> {
    let line = r.line()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    let num = |s: &str| s.parse::<usize>().map_err(|_| format!("bad resolution line {line:?}"));
    let (w, h, bottom_up) = match parts.as_slice() {
        ["-Y", h, "+X", w] => (num(w)?, num(h)?, false),
        ["+Y", h, "+X", w] => (num(w)?, num(h)?, true),
        _ => return Err(format!("unsupported Radiance orientation {line:?} (only -Y/+Y with +X)")),
    };
    if w == 0 || h == 0 || w.saturating_mul(h) > 1 << 28 {
        return Err(format!("unusable image size {w} × {h}"));
    }
    Ok((w, h, bottom_up))
}

fn read_scanline(r: &mut Reader, line: &mut [[u8; 4]]) -> Result<(), String> {
    let w = line.len();
    let start = r.at;
    let first = r.bytes4()?;
    if (8..0x8000).contains(&w) && first[0] == 2 && first[1] == 2 && first[2] & 0x80 == 0 {
        if ((first[2] as usize) << 8 | first[3] as usize) != w {
            return Err("a run-length scanline has the wrong width".into());
        }
        for c in 0..4 {
            let mut x = 0;
            while x < w {
                let count = r.byte()? as usize;
                if count > 128 {
                    let n = count - 128;
                    let v = r.byte()?;
                    for p in line.get_mut(x..x + n).ok_or("a run passes the end of the scanline")? {
                        p[c] = v;
                    }
                    x += n;
                } else {
                    if count == 0 {
                        return Err("an empty run in a scanline".into());
                    }
                    for p in line.get_mut(x..x + count).ok_or("a run passes the end of the scanline")? {
                        p[c] = r.byte()?;
                    }
                    x += count;
                }
            }
        }
        return Ok(());
    }
    // flat pixels, possibly with old-style runs
    r.at = start;
    let mut x = 0;
    let mut shift = 0;
    while x < w {
        let p = r.bytes4()?;
        if p[0] == 1 && p[1] == 1 && p[2] == 1 {
            let prev = if x > 0 { line.get(x - 1).copied().unwrap_or_default() } else { return Err("a repeat at the start of a scanline".into()) };
            let n = (p[3] as usize) << shift;
            for q in line.get_mut(x..x + n).ok_or("a repeat passes the end of the scanline")? {
                *q = prev;
            }
            x += n;
            shift += 8;
            if shift > 16 {
                return Err("a repeat too long for the scanline".into());
            }
        } else {
            if let Some(q) = line.get_mut(x) {
                *q = p;
            }
            x += 1;
            shift = 0;
        }
    }
    Ok(())
}

fn rgbe_to_float(p: [u8; 4]) -> [f32; 3] {
    if p[3] == 0 {
        return [0.0; 3];
    }
    let f = 2f32.powi(p[3] as i32 - (128 + 8));
    [p[0] as f32 * f, p[1] as f32 * f, p[2] as f32 * f]
}

/// CIE XYZ to linear sRGB (D65).
fn xyz_to_srgb(c: [f32; 3]) -> [f32; 3] {
    [
        3.240_454 * c[0] - 1.537_139 * c[1] - 0.498_531 * c[2],
        -0.969_266 * c[0] + 1.876_011 * c[1] + 0.041_556 * c[2],
        0.055_643 * c[0] - 0.204_026 * c[1] + 1.057_225 * c[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exr::encode_hdr;

    fn sample(w: usize, h: usize) -> Tex {
        Tex::from_fn(w, h, |u, v| [u * 8.0 + 0.01, v * 0.5, (u * v) * 1000.0, 1.0])
    }

    fn close(a: &Tex, b: &Tex) -> bool {
        a.w == b.w
            && a.h == b.h
            && a.px.iter().zip(&b.px).all(|(p, q)| (0..3).all(|c| (p[c] - q[c]).abs() <= q[0].max(q[1]).max(q[2]) * 0.016 + 1e-6 && p[3] == 1.0))
    }

    /// The per-component run-length encoding of one RGBE scanline (as most writers produce it).
    fn rle_line(line: &[[u8; 4]], out: &mut Vec<u8>) {
        let w = line.len();
        out.extend_from_slice(&[2, 2, (w >> 8) as u8, (w & 255) as u8]);
        for c in 0..4 {
            let comp: Vec<u8> = line.iter().map(|p| p[c]).collect();
            let mut x = 0;
            while x < w {
                let mut run = 1;
                while x + run < w && run < 127 && comp[x + run] == comp[x] {
                    run += 1;
                }
                if run >= 3 {
                    out.extend_from_slice(&[128 + run as u8, comp[x]]);
                    x += run;
                } else {
                    let mut n = 0;
                    while x + n < w && n < 128 && !(x + n + 2 < w && comp[x + n] == comp[x + n + 1] && comp[x + n] == comp[x + n + 2]) {
                        n += 1;
                    }
                    let n = n.max(1);
                    out.push(n as u8);
                    out.extend_from_slice(&comp[x..x + n]);
                    x += n;
                }
            }
        }
    }

    #[test]
    fn flat_files_from_our_writer_round_trip() {
        let t = sample(13, 7);
        let back = decode(&encode_hdr(&t)).unwrap();
        assert!(close(&back, &t));
        assert!(sniff(&encode_hdr(&t)));
    }

    #[test]
    fn run_length_scanlines_and_bottom_up_files_read() {
        let t = sample(40, 5);
        let flat = encode_hdr(&t);
        // re-encode the pixels as run-length scanlines (with a constant region so runs occur)
        let header_end = flat.windows(2).position(|w| w == b"\n\n").unwrap() + 2;
        let res_end = header_end + flat[header_end..].iter().position(|&c| c == b'\n').unwrap() + 1;
        let mut px: Vec<[u8; 4]> = flat[res_end..].chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        for p in px.iter_mut().take(20) {
            *p = [10, 20, 30, 129];
        }
        let mut rle = flat[..res_end].to_vec();
        for line in px.chunks(40) {
            rle_line(line, &mut rle);
        }
        let back = decode(&rle).unwrap();
        let flat_back = {
            let mut f = flat[..res_end].to_vec();
            px.iter().for_each(|p| f.extend_from_slice(p));
            decode(&f).unwrap()
        };
        assert_eq!(back.px, flat_back.px);
        assert_eq!(back.px[0], [10.0 / 128.0, 20.0 / 128.0, 30.0 / 128.0, 1.0]);
        // +Y: the first scanline is the bottom row
        let mut up = b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n+Y 2 +X 1\n".to_vec();
        up.extend_from_slice(&[128, 0, 0, 129, 0, 128, 0, 129]);
        let t = decode(&up).unwrap();
        assert_eq!((t.px[0], t.px[1]), ([0.0, 1.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn old_style_repeats_expand() {
        let mut f = b"#?RGBE\n\n-Y 1 +X 5\n".to_vec();
        f.extend_from_slice(&[64, 64, 64, 129, 1, 1, 1, 3, 0, 0, 0, 0]);
        let t = decode(&f).unwrap();
        assert_eq!(t.px.iter().filter(|p| p[0] == 0.5).count(), 4);
        assert_eq!(t.px[4], [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn damaged_files_are_errors_not_panics() {
        let t = sample(16, 4);
        let good = encode_hdr(&t);
        for cut in [0, 5, 20, good.len() / 2, good.len() - 1] {
            assert!(decode(&good[..cut]).is_err(), "cut at {cut}");
        }
        assert!(decode(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 0 +X 4\n").is_err());
        assert!(decode(b"#?RADIANCE\nFORMAT=48-bit\n\n-Y 1 +X 1\n\0\0\0\0").is_err());
        assert!(decode(b"#?RADIANCE\n\n+X 1 -Y 1\n\0\0\0\0").is_err());
        // a run-length scanline whose runs overflow the width
        let mut bad = b"#?RADIANCE\n\n-Y 1 +X 8\n".to_vec();
        bad.extend_from_slice(&[2, 2, 0, 8, 128 + 100, 7]);
        assert!(decode(&bad).is_err());
        // random bytes after a valid header never panic
        let mut seed = 12345u32;
        for _ in 0..200 {
            let mut f = b"#?RADIANCE\n\n-Y 3 +X 9\n".to_vec();
            for _ in 0..40 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                f.push((seed >> 24) as u8);
            }
            let _ = decode(&f);
        }
    }
}
