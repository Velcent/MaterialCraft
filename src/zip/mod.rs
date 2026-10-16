//! ZIP archives: writing (stored or deflate, ZIP64 when needed) and reading (stored and deflate entries, ZIP64).
//!
//! ```
//! use materialcraft::zip::{Method, ZipReader, ZipWriter};
//! let mut w = ZipWriter::in_memory();
//! w.add("document.json", b"{}", Method::Deflate).unwrap();
//! let bytes = w.finish_bytes().unwrap();
//! let mut r = ZipReader::open(std::io::Cursor::new(bytes)).unwrap();
//! assert_eq!(r.read("document.json").unwrap(), b"{}");
//! ```

mod format;
mod reader;
mod writer;

pub use reader::{ZipEntry, ZipReader};
pub use writer::ZipWriter;

/// Entry compression method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Stored,
    Deflate,
}

#[derive(Debug)]
pub enum ZipError {
    Io(std::io::Error),
    /// Malformed archive or invalid argument.
    Format(String),
    /// Valid ZIP feature this implementation does not handle (encryption, other methods, multi-disk).
    Unsupported(String),
    /// CRC-32 mismatch in the named entry.
    Checksum(String),
    NotFound(String),
}

impl std::fmt::Display for ZipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZipError::Io(e) => write!(f, "ZIP I/O error: {e}"),
            ZipError::Format(m) => write!(f, "invalid ZIP: {m}"),
            ZipError::Unsupported(m) => write!(f, "unsupported ZIP feature: {m}"),
            ZipError::Checksum(n) => write!(f, "CRC mismatch in ZIP entry '{n}'"),
            ZipError::NotFound(n) => write!(f, "no entry '{n}' in ZIP archive"),
        }
    }
}

impl std::error::Error for ZipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ZipError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ZipError {
    fn from(e: std::io::Error) -> Self {
        ZipError::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample() -> Vec<(String, Vec<u8>, Method)> {
        let text: Vec<u8> = b"layer mask roughness "
            .iter()
            .cycle()
            .take(100_000)
            .copied()
            .collect();
        let mut s = 99u32;
        let noise: Vec<u8> = (0..5000)
            .map(|_| {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (s >> 24) as u8
            })
            .collect();
        vec![
            (
                "document.json".into(),
                b"{\"version\": 1}".to_vec(),
                Method::Deflate,
            ),
            ("textures/".into(), vec![], Method::Stored),
            (
                "textures/vase/base_color.png".into(),
                noise,
                Method::Deflate,
            ),
            ("big.txt".into(), text, Method::Deflate),
            ("empty".into(), vec![], Method::Deflate),
            ("ümlaut/名前.txt".into(), b"utf8".to_vec(), Method::Stored),
        ]
    }

    fn build(force_zip64: bool) -> Vec<u8> {
        let mut w = ZipWriter::in_memory();
        w.set_force_zip64(force_zip64);
        w.set_modified(1_700_000_000);
        for (n, d, m) in sample() {
            w.add(&n, &d, m).unwrap();
        }
        w.finish_bytes().unwrap()
    }

    #[test]
    fn round_trip() {
        for zip64 in [false, true] {
            let bytes = build(zip64);
            let mut r = ZipReader::open(Cursor::new(bytes.clone())).unwrap();
            let names: Vec<String> = r.entries().iter().map(|e| e.name.clone()).collect();
            assert_eq!(names, sample().into_iter().map(|s| s.0).collect::<Vec<_>>());
            for (n, d, _) in sample() {
                assert_eq!(r.read(&n).unwrap(), d, "{n}");
            }
            assert!(r.find("textures/").unwrap().is_dir());
            assert_eq!(r.find("big.txt").unwrap().method(), Some(Method::Deflate));
            // Incompressible data was stored despite asking for deflate.
            assert_eq!(
                r.find("textures/vase/base_color.png").unwrap().method(),
                Some(Method::Stored)
            );
            assert!(matches!(r.read("nope"), Err(ZipError::NotFound(_))));
        }
    }

    #[test]
    fn empty_archive() {
        let bytes = ZipWriter::in_memory().finish_bytes().unwrap();
        assert_eq!(bytes.len(), 22);
        assert!(ZipReader::open(Cursor::new(bytes))
            .unwrap()
            .entries()
            .is_empty());
    }

    #[test]
    fn rejects_bad_names() {
        let mut w = ZipWriter::in_memory();
        assert!(w.add("", b"", Method::Stored).is_err());
        assert!(w.add("/abs", b"", Method::Stored).is_err());
        assert!(w.add("a\\b", b"", Method::Stored).is_err());
        w.add("a", b"", Method::Stored).unwrap();
        assert!(w.add("a", b"", Method::Stored).is_err());
    }

    #[test]
    fn many_entries_use_zip64() {
        let mut w = ZipWriter::in_memory();
        for i in 0..70_000 {
            w.add(&format!("f{i}"), &[i as u8], Method::Stored).unwrap();
        }
        let bytes = w.finish_bytes().unwrap();
        let mut r = ZipReader::open(Cursor::new(bytes)).unwrap();
        assert_eq!(r.entries().len(), 70_000);
        assert_eq!(r.read("f69999").unwrap(), [69_999u32 as u8]);
    }

    #[test]
    fn detects_corruption() {
        let mut bytes = build(false);
        // Flip a byte inside the stored "utf8" payload.
        let at = bytes.windows(4).position(|w| w == b"utf8").unwrap();
        bytes[at] ^= 1;
        let mut r = ZipReader::open(Cursor::new(bytes)).unwrap();
        assert!(matches!(
            r.read("ümlaut/名前.txt"),
            Err(ZipError::Checksum(_))
        ));
        assert!(r.read("document.json").is_ok());
    }

    #[test]
    fn rejects_encrypted_and_unknown_methods() {
        let bytes = build(false);
        let cd = bytes
            .windows(4)
            .position(|w| w == [0x50, 0x4b, 0x01, 0x02])
            .unwrap();
        let mut enc = bytes.clone();
        enc[cd + 8] |= 1; // general-purpose flags: encrypted
        let mut r = ZipReader::open(Cursor::new(enc)).unwrap();
        assert!(matches!(
            r.read("document.json"),
            Err(ZipError::Unsupported(_))
        ));
        let mut lzma = bytes;
        lzma[cd + 10] = 14; // method: LZMA
        let mut r = ZipReader::open(Cursor::new(lzma)).unwrap();
        assert_eq!(r.entries()[0].method(), None);
        assert!(matches!(
            r.read("document.json"),
            Err(ZipError::Unsupported(_))
        ));
    }

    #[test]
    fn truncations_and_garbage_error() {
        for zip64 in [false, true] {
            let bytes = build(zip64);
            for n in 0..bytes.len() {
                let r = ZipReader::open(Cursor::new(&bytes[..n]));
                // A prefix can only open if it still ends in a valid directory, which none of ours does.
                assert!(r.is_err(), "prefix {n}");
            }
        }
        let mut s = 5u32;
        for len in [0usize, 10, 22, 100, 70_000] {
            let mut g: Vec<u8> = (0..len)
                .map(|_| {
                    s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    (s >> 16) as u8
                })
                .collect();
            assert!(ZipReader::open(Cursor::new(g.clone())).is_err());
            // Garbage with a plausible end record pointing nowhere.
            g.extend_from_slice(&[
                0x50, 0x4b, 5, 6, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f, 0, 0,
                0, 0, 0, 0,
            ]);
            assert!(ZipReader::open(Cursor::new(g)).is_err());
        }
    }
}
