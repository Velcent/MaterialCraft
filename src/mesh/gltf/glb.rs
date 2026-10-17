//! The GLB container: a 12-byte header, a JSON chunk and an optional binary (BIN) chunk.

use super::error::GltfError;

const MAGIC: &[u8; 4] = b"glTF";
const VERSION: u32 = 2;
const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;

/// The two chunks of a GLB file, borrowed from its bytes.
pub(super) struct Glb<'a> {
    pub json: &'a [u8],
    pub bin: Option<&'a [u8]>,
}

impl<'a> Glb<'a> {
    /// Splits a GLB file into its chunks. Chunks after BIN (allowed by the spec) are ignored.
    pub fn parse(bytes: &'a [u8]) -> Result<Glb<'a>, GltfError> {
        let bad = |m: &str| GltfError::Container(m.to_string());
        if bytes.len() < 12 || &bytes[..4] != MAGIC {
            return Err(bad("not a GLB file (no glTF header)"));
        }
        let version = u32_at(bytes, 4).ok_or_else(|| bad("truncated header"))?;
        if version != VERSION {
            return Err(GltfError::Unsupported(format!("GLB version {version}")));
        }
        let length = u32_at(bytes, 8).ok_or_else(|| bad("truncated header"))? as usize;
        if length > bytes.len() || length < 12 {
            return Err(bad("declared length does not match the data"));
        }
        let bytes = &bytes[..length];
        let mut chunks = Vec::new();
        let mut pos = 12;
        while pos < bytes.len() {
            let (Some(len), Some(kind)) = (u32_at(bytes, pos), u32_at(bytes, pos + 4)) else {
                return Err(bad("truncated chunk header"));
            };
            let start = pos + 8;
            let end = start.checked_add(len as usize).filter(|&e| e <= bytes.len()).ok_or_else(|| bad("chunk extends past the end of the file"))?;
            chunks.push((kind, &bytes[start..end]));
            pos = end;
        }
        match chunks.as_slice() {
            [(JSON_CHUNK, json), rest @ ..] => Ok(Glb {
                json,
                bin: match rest.first() {
                    Some((BIN_CHUNK, bin)) => Some(bin),
                    _ => None,
                },
            }),
            _ => Err(bad("the first chunk must be JSON")),
        }
    }

    /// Assembles a GLB file; pads JSON with spaces and BIN with zeros to 4-byte boundaries.
    pub fn assemble(json: &str, bin: &[u8]) -> Result<Vec<u8>, GltfError> {
        let pad = |n: usize| (4 - n % 4) % 4;
        let json_len = json.len() + pad(json.len());
        let bin_len = bin.len() + pad(bin.len());
        let total = 12 + 8 + json_len + if bin.is_empty() { 0 } else { 8 + bin_len };
        let total32 = u32::try_from(total).map_err(|_| GltfError::Unsupported("meshes of 4 GiB or more".into()))?;
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&total32.to_le_bytes());
        out.extend_from_slice(&(json_len as u32).to_le_bytes());
        out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
        out.extend_from_slice(json.as_bytes());
        out.resize(out.len() + pad(json.len()), b' ');
        if !bin.is_empty() {
            out.extend_from_slice(&(bin_len as u32).to_le_bytes());
            out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
            out.extend_from_slice(bin);
            out.resize(out.len() + pad(bin.len()), 0);
        }
        Ok(out)
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assemble_and_parse() {
        let glb = Glb::assemble("{}", &[1, 2, 3]).unwrap();
        assert_eq!(glb.len() % 4, 0);
        let g = Glb::parse(&glb).unwrap();
        assert_eq!(g.json, b"{}  ");
        assert_eq!(g.bin, Some(&[1u8, 2, 3, 0][..]));
        let no_bin = Glb::assemble("{\"a\":1}", &[]).unwrap();
        assert!(Glb::parse(&no_bin).unwrap().bin.is_none());
        for n in 0..glb.len() {
            assert!(Glb::parse(&glb[..n]).is_err(), "prefix {n}");
        }
    }
}
