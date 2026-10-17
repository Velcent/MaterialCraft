//! ZIP reader over any `Read + Seek`: loads only the central directory up front, then individual entries on demand.

use super::format::*;
use super::{Method, ZipError};
use crate::checksum::crc32;
use std::io::{Read, Seek, SeekFrom};

/// One entry of the central directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipEntry {
    /// Path inside the archive, '/'-separated (names not flagged UTF-8 are decoded lossily).
    pub name: String,
    /// Raw method code (0 stored, 8 deflate; others are listed but cannot be read).
    pub method_code: u16,
    pub size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub encrypted: bool,
    local_header_offset: u64,
}

impl ZipEntry {
    pub fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }

    /// The method, if this reader supports it.
    pub fn method(&self) -> Option<Method> {
        match self.method_code {
            METHOD_STORED => Some(Method::Stored),
            METHOD_DEFLATE => Some(Method::Deflate),
            _ => None,
        }
    }
}

/// Reads a ZIP archive. Single-disk archives only (no spanning); data before the archive (self-extractors) is not
/// supported.
pub struct ZipReader<R: Read + Seek> {
    src: R,
    len: u64,
    entries: Vec<ZipEntry>,
}

/// Where the central directory is, from the end records.
struct Directory {
    count: u64,
    size: u64,
    offset: u64,
}

fn bad(msg: &str) -> ZipError {
    ZipError::Format(msg.to_string())
}

impl<R: Read + Seek> ZipReader<R> {
    /// Read the central directory of the archive in `src`.
    pub fn open(mut src: R) -> Result<Self, ZipError> {
        let len = src.seek(SeekFrom::End(0))?;
        let dir = Self::find_directory(&mut src, len)?;
        let entries = Self::read_directory(&mut src, len, &dir)?;
        Ok(ZipReader { src, len, entries })
    }

    fn read_at(src: &mut R, offset: u64, n: usize) -> Result<Vec<u8>, ZipError> {
        src.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; n];
        src.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn find_directory(src: &mut R, len: u64) -> Result<Directory, ZipError> {
        // The end record is the last 22 bytes plus a comment of at most 64 KiB.
        let tail_len = len.min((EOCD_LEN + u16::MAX as usize) as u64) as usize;
        let tail_start = len - tail_len as u64;
        let tail = Self::read_at(src, tail_start, tail_len)?;
        // Prefer a record whose comment ends exactly at the end of the file (a comment may itself contain the
        // signature); accept trailing junk only if no such record exists.
        let candidates = || (0..tail_len.saturating_sub(EOCD_LEN - 1)).rev().filter(|&i| tail[i..i + 4] == EOCD_SIG.to_le_bytes());
        let end_of = |i: usize| i + EOCD_LEN + u16::from_le_bytes([tail[i + 20], tail[i + 21]]) as usize;
        let at = candidates()
            .find(|&i| end_of(i) == tail_len)
            .or_else(|| candidates().find(|&i| end_of(i) <= tail_len))
            .ok_or_else(|| bad("not a ZIP archive (no end of central directory record)"))?;
        let mut f = Fields::new(&tail[at + 4..]);
        let (disk, cd_disk) = (f.u16(), f.u16());
        let _entries_this_disk = f.u16();
        let (count, size, offset) = (f.u16(), f.u32(), f.u32());
        let (Some(disk), Some(cd_disk), Some(count), Some(size), Some(offset)) = (disk, cd_disk, count, size, offset) else {
            return Err(bad("truncated end record"));
        };
        if disk != 0 || cd_disk != 0 {
            return Err(ZipError::Unsupported("multi-disk archives".into()));
        }
        let mut dir = Directory { count: count as u64, size: size as u64, offset: offset as u64 };

        // A ZIP64 locator immediately before the end record points at the ZIP64 end record.
        let eocd_pos = tail_start + at as u64;
        if eocd_pos >= ZIP64_LOCATOR_LEN as u64 {
            let loc = Self::read_at(src, eocd_pos - ZIP64_LOCATOR_LEN as u64, ZIP64_LOCATOR_LEN)?;
            let mut f = Fields::new(&loc);
            if f.u32() == Some(ZIP64_LOCATOR_SIG) {
                let _disk = f.u32();
                let rec_offset = f.u64().ok_or_else(|| bad("truncated ZIP64 locator"))?;
                if rec_offset.checked_add(ZIP64_EOCD_LEN as u64).is_none_or(|end| end > len) {
                    return Err(bad("ZIP64 end record out of range"));
                }
                let rec = Self::read_at(src, rec_offset, ZIP64_EOCD_LEN)?;
                let mut f = Fields::new(&rec);
                if f.u32() != Some(ZIP64_EOCD_SIG) {
                    return Err(bad("bad ZIP64 end record"));
                }
                f.bytes(8 + 2 + 2 + 4 + 4 + 8);
                let (Some(count), Some(size), Some(offset)) = (f.u64(), f.u64(), f.u64()) else {
                    return Err(bad("truncated ZIP64 end record"));
                };
                dir = Directory { count, size, offset };
            }
        }
        if dir.offset.checked_add(dir.size).is_none_or(|end| end > len) {
            return Err(bad("central directory out of range"));
        }
        if dir.count > dir.size / CENTRAL_HEADER_LEN as u64 {
            return Err(bad("entry count does not fit the central directory"));
        }
        Ok(dir)
    }

    fn read_directory(src: &mut R, len: u64, dir: &Directory) -> Result<Vec<ZipEntry>, ZipError> {
        // Bounded by the file length (checked above).
        let cd = Self::read_at(src, dir.offset, dir.size as usize)?;
        let mut f = Fields::new(&cd);
        let mut entries = Vec::with_capacity(dir.count as usize);
        for _ in 0..dir.count {
            let e = Self::central_entry(&mut f).ok_or_else(|| bad("truncated central directory"))??;
            if e.local_header_offset >= len {
                return Err(bad("entry offset out of range"));
            }
            entries.push(e);
        }
        Ok(entries)
    }

    /// Parse one central header. Outer `None`: ran out of data; inner error: malformed record.
    fn central_entry(f: &mut Fields) -> Option<Result<ZipEntry, ZipError>> {
        if f.u32()? != CENTRAL_HEADER_SIG {
            return Some(Err(bad("bad central directory signature")));
        }
        f.bytes(4)?; // versions
        let flags = f.u16()?;
        let method_code = f.u16()?;
        f.bytes(4)?; // time, date
        let crc = f.u32()?;
        let (csize, size) = (f.u32()? as u64, f.u32()? as u64);
        let (name_len, extra_len, comment_len) = (f.u16()? as usize, f.u16()? as usize, f.u16()? as usize);
        f.bytes(8)?; // disk, internal and external attributes
        let offset = f.u32()? as u64;
        let name = f.bytes(name_len)?;
        let extra = f.bytes(extra_len)?;
        f.bytes(comment_len)?;
        let (ns, nc, no) = (size == U32_SENTINEL, csize == U32_SENTINEL, offset == U32_SENTINEL);
        let z = if ns || nc || no {
            match Zip64Extra::decode(extra, ns, nc, no) {
                Some(z) => z,
                None => return Some(Err(bad("missing ZIP64 extra field"))),
            }
        } else {
            Zip64Extra::default()
        };
        Some(Ok(ZipEntry {
            name: String::from_utf8_lossy(name).into_owned(),
            method_code,
            size: z.size.unwrap_or(size),
            compressed_size: z.compressed_size.unwrap_or(csize),
            crc32: crc,
            encrypted: flags & FLAG_ENCRYPTED != 0,
            local_header_offset: z.local_header_offset.unwrap_or(offset),
        }))
    }

    /// All entries, in central-directory order.
    pub fn entries(&self) -> &[ZipEntry] {
        &self.entries
    }

    pub fn find(&self, name: &str) -> Option<&ZipEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Read and decompress the entry called `name`, verifying its size and CRC-32.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, ZipError> {
        let e = self.find(name).cloned().ok_or_else(|| ZipError::NotFound(name.to_string()))?;
        self.read_entry(&e)
    }

    /// Read and decompress `e` (an entry of this archive), verifying its size and CRC-32.
    pub fn read_entry(&mut self, e: &ZipEntry) -> Result<Vec<u8>, ZipError> {
        if e.encrypted {
            return Err(ZipError::Unsupported(format!("'{}' is encrypted", e.name)));
        }
        let method = e.method().ok_or_else(|| ZipError::Unsupported(format!("'{}' uses compression method {}", e.name, e.method_code)))?;
        let header = Self::read_at(&mut self.src, e.local_header_offset, LOCAL_HEADER_LEN)?;
        let mut f = Fields::new(&header);
        if f.u32() != Some(LOCAL_HEADER_SIG) {
            return Err(bad("bad local header signature"));
        }
        f.bytes(22);
        let (Some(name_len), Some(extra_len)) = (f.u16(), f.u16()) else {
            return Err(bad("truncated local header"));
        };
        let start = e.local_header_offset + (LOCAL_HEADER_LEN + name_len as usize + extra_len as usize) as u64;
        if start.checked_add(e.compressed_size).is_none_or(|end| end > self.len) {
            return Err(bad("entry data out of range"));
        }
        let raw = Self::read_at(&mut self.src, start, e.compressed_size as usize)?;
        let data = match method {
            Method::Stored => raw,
            Method::Deflate => crate::deflate::inflate(&raw).map_err(|m| ZipError::Format(format!("'{}': {m}", e.name)))?,
        };
        if data.len() as u64 != e.size {
            return Err(ZipError::Format(format!("'{}': size mismatch", e.name)));
        }
        if crc32(&data) != e.crc32 {
            return Err(ZipError::Checksum(e.name.clone()));
        }
        Ok(data)
    }

    pub fn into_inner(self) -> R {
        self.src
    }
}
