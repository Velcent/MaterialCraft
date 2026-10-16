//! The file header: magic number, version flags and the attribute list.

use super::{Compression, SampleKind};

const MAGIC: [u8; 4] = [0x76, 0x2f, 0x31, 0x01];
const VERSION: u32 = 2;
const FLAG_TILED: u32 = 0x200;
const FLAG_LONG_NAMES: u32 = 0x400;
const FLAG_DEEP: u32 = 0x800;
const FLAG_MULTIPART: u32 = 0x1000;
/// Attribute and channel names longer than this need the long-names flag.
const SHORT_NAME_MAX: usize = 31;
const LONG_NAME_MAX: usize = 255;

/// What the codec needs from a header.
#[derive(Debug, PartialEq)]
pub(super) struct Header {
    /// Sorted by name, as stored.
    pub channels: Vec<(String, SampleKind)>,
    pub compression: Compression,
    /// Data window: x_min, y_min, x_max, y_max (inclusive).
    pub data_window: [i32; 4],
}

impl Header {
    pub fn width(&self) -> usize {
        (self.data_window[2] as i64 - self.data_window[0] as i64 + 1) as usize
    }

    pub fn height(&self) -> usize {
        (self.data_window[3] as i64 - self.data_window[1] as i64 + 1) as usize
    }

    /// Bytes of one scanline across all channels.
    pub fn line_bytes(&self) -> usize {
        self.width() * self.channels.iter().map(|(_, k)| k.size()).sum::<usize>()
    }

    /// Magic, version and all required attributes (alphabetical, as OpenEXR writes them).
    pub fn write(&self, out: &mut Vec<u8>) {
        let long = self.channels.iter().any(|(n, _)| n.len() > SHORT_NAME_MAX);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&(VERSION | if long { FLAG_LONG_NAMES } else { 0 }).to_le_bytes());

        let mut chlist = Vec::new();
        for (name, kind) in &self.channels {
            chlist.extend_from_slice(name.as_bytes());
            chlist.push(0);
            chlist.extend_from_slice(&kind.code().to_le_bytes());
            chlist.extend_from_slice(&[0, 0, 0, 0]); // pLinear, reserved
            chlist.extend_from_slice(&1i32.to_le_bytes()); // xSampling
            chlist.extend_from_slice(&1i32.to_le_bytes()); // ySampling
        }
        chlist.push(0);
        let window: Vec<u8> = self
            .data_window
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        attribute(out, "channels", "chlist", &chlist);
        attribute(
            out,
            "compression",
            "compression",
            &[self.compression.code()],
        );
        attribute(out, "dataWindow", "box2i", &window);
        attribute(out, "displayWindow", "box2i", &window);
        attribute(out, "lineOrder", "lineOrder", &[0]); // INCREASING_Y
        attribute(out, "pixelAspectRatio", "float", &1f32.to_le_bytes());
        attribute(out, "screenWindowCenter", "v2f", &[0; 8]);
        attribute(out, "screenWindowWidth", "float", &1f32.to_le_bytes());
        out.push(0);
    }

    /// Parse the header at the start of `b`; returns it and the offset of the line-offset table.
    pub fn read(b: &[u8]) -> Result<(Header, usize), String> {
        if b.len() < 8 || b[..4] != MAGIC {
            return Err("not an OpenEXR file".into());
        }
        let flags = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        if flags & 0xff != VERSION {
            return Err(format!("unsupported OpenEXR version {}", flags & 0xff));
        }
        if flags & (FLAG_TILED | FLAG_DEEP | FLAG_MULTIPART) != 0 {
            return Err("only single-part scanline OpenEXR files are supported".into());
        }
        let name_max = if flags & FLAG_LONG_NAMES != 0 {
            LONG_NAME_MAX
        } else {
            SHORT_NAME_MAX
        };
        let mut pos = 8;
        let (mut channels, mut compression, mut data_window) = (None, None, None);
        loop {
            let name = cstr(b, &mut pos, name_max)?;
            if name.is_empty() {
                break;
            }
            let kind = cstr(b, &mut pos, name_max)?;
            let size = i32::from_le_bytes(
                take(b, &mut pos, 4)?
                    .try_into()
                    .map_err(|_| "bad attribute size")?,
            );
            let size = usize::try_from(size).map_err(|_| "negative attribute size")?;
            let value = take(b, &mut pos, size)?;
            match (name, kind) {
                ("channels", "chlist") => channels = Some(parse_chlist(value, name_max)?),
                ("compression", "compression") if size == 1 => {
                    compression = Some(Compression::from_code(value[0])?)
                }
                ("dataWindow", "box2i") if size == 16 => {
                    let v: Vec<i32> = value
                        .chunks_exact(4)
                        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                        .collect();
                    data_window = Some([v[0], v[1], v[2], v[3]]);
                }
                ("type", "string") if value != b"scanlineimage" => {
                    return Err("only scanline images are supported".into())
                }
                _ => {}
            }
        }
        let channels = channels.ok_or("missing channels attribute")?;
        let compression = compression.ok_or("missing compression attribute")?;
        let data_window = data_window.ok_or("missing dataWindow attribute")?;
        if data_window[2] < data_window[0] || data_window[3] < data_window[1] {
            return Err("empty data window".into());
        }
        if channels.is_empty() {
            return Err("no channels".into());
        }
        Ok((
            Header {
                channels,
                compression,
                data_window,
            },
            pos,
        ))
    }
}

fn attribute(out: &mut Vec<u8>, name: &str, kind: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out.extend_from_slice(kind.as_bytes());
    out.push(0);
    out.extend_from_slice(&(value.len() as i32).to_le_bytes());
    out.extend_from_slice(value);
}

fn take<'a>(b: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], String> {
    let s = b
        .get(*pos..pos.checked_add(n).ok_or("bad length")?)
        .ok_or("truncated header")?;
    *pos += n;
    Ok(s)
}

/// A NUL-terminated name of at most `max` bytes.
fn cstr<'a>(b: &'a [u8], pos: &mut usize, max: usize) -> Result<&'a str, String> {
    let rest = b.get(*pos..).ok_or("truncated header")?;
    let len = rest
        .iter()
        .take(max + 1)
        .position(|&c| c == 0)
        .ok_or("truncated header or name too long")?;
    let s = std::str::from_utf8(&rest[..len]).map_err(|_| "name is not UTF-8")?;
    *pos += len + 1;
    Ok(s)
}

fn parse_chlist(v: &[u8], name_max: usize) -> Result<Vec<(String, SampleKind)>, String> {
    let mut pos = 0;
    let mut out: Vec<(String, SampleKind)> = Vec::new();
    loop {
        let name = cstr(v, &mut pos, name_max)?;
        if name.is_empty() {
            break;
        }
        let f = take(v, &mut pos, 16)?;
        let word = |i: usize| i32::from_le_bytes([f[i], f[i + 1], f[i + 2], f[i + 3]]);
        let kind = SampleKind::from_code(word(0))
            .ok_or_else(|| format!("channel '{name}': unknown pixel type"))?;
        if word(8) != 1 || word(12) != 1 {
            return Err(format!(
                "channel '{name}': subsampled channels are not supported"
            ));
        }
        out.push((name.to_string(), kind));
    }
    // Pixel data is laid out in name order whatever order the list was written in (OpenEXR keeps it in a map).
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    if out.windows(2).any(|p| p[0].0 == p[1].0) {
        return Err("duplicate channel name".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trip() {
        let h = Header {
            channels: vec![
                ("B".into(), SampleKind::Half),
                ("G".into(), SampleKind::Float),
                ("R".into(), SampleKind::Uint),
            ],
            compression: Compression::Zip,
            data_window: [0, 0, 99, 49],
        };
        let mut b = Vec::new();
        h.write(&mut b);
        let (back, end) = Header::read(&b).unwrap();
        assert_eq!(back, h);
        assert_eq!(end, b.len());
        assert_eq!(
            (back.width(), back.height(), back.line_bytes()),
            (100, 50, 1000)
        );
        for n in 0..b.len() {
            assert!(Header::read(&b[..n]).is_err());
        }
    }
}
