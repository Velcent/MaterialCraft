//! Streaming ZIP writer. Each entry is compressed in memory first, so sizes and CRC are known before its local
//! header is written: no data descriptors and no seeking, any `Write` works.

use super::format::*;
use super::{Method, ZipError};
use crate::checksum::crc32;
use crate::deflate::{self, Level};
use std::io::{Cursor, Write};

/// What the central directory needs to know about a written entry.
struct Written {
    name: String,
    method: u16,
    flags: u16,
    crc: u32,
    size: u64,
    compressed_size: u64,
    offset: u64,
    time: u16,
    date: u16,
}

/// Writes a ZIP archive entry by entry; call [`ZipWriter::finish`] to append the central directory.
pub struct ZipWriter<W: Write> {
    out: W,
    /// Bytes written so far: the offset of the next record.
    pos: u64,
    entries: Vec<Written>,
    level: Level,
    dos_time: (u16, u16),
    force_zip64: bool,
}

impl ZipWriter<Cursor<Vec<u8>>> {
    /// A writer that builds the archive in memory; get the bytes with [`ZipWriter::finish_bytes`].
    pub fn in_memory() -> Self {
        ZipWriter::new(Cursor::new(Vec::new()))
    }

    pub fn finish_bytes(self) -> Result<Vec<u8>, ZipError> {
        Ok(self.finish()?.into_inner())
    }
}

impl<W: Write> ZipWriter<W> {
    /// Start an archive at the current position of `out` (offsets are counted from there).
    pub fn new(out: W) -> Self {
        ZipWriter { out, pos: 0, entries: Vec::new(), level: Level::Default, dos_time: dos_datetime(0), force_zip64: false }
    }

    /// Effort used for `Method::Deflate` entries (default `Level::Default`).
    pub fn set_level(&mut self, level: Level) {
        self.level = level;
    }

    /// Modification time stamped on entries added from now on (Unix seconds, UTC). Defaults to 1980-01-01 so that
    /// archives are reproducible.
    pub fn set_modified(&mut self, unix_secs: i64) {
        self.dos_time = dos_datetime(unix_secs);
    }

    /// Write ZIP64 records even when nothing needs them (for testing readers; normally ZIP64 is used only when
    /// sizes or offsets reach 4 GiB or there are more than 65535 entries).
    pub fn set_force_zip64(&mut self, on: bool) {
        self.force_zip64 = on;
    }

    /// Number of entries added so far.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add a file. `name` uses '/' separators; a trailing '/' makes a directory entry (pass empty data).
    /// A `Deflate` entry that would not shrink is stored instead.
    pub fn add(&mut self, name: &str, data: &[u8], method: Method) -> Result<(), ZipError> {
        if name.is_empty() || name.len() > u16::MAX as usize || name.starts_with('/') || name.contains('\\') {
            return Err(ZipError::Format(format!("invalid entry name '{name}'")));
        }
        if self.entries.iter().any(|e| e.name == name) {
            return Err(ZipError::Format(format!("duplicate entry name '{name}'")));
        }
        let compressed = match method {
            Method::Deflate => Some(deflate::compress(data, self.level)).filter(|z| z.len() < data.len()),
            Method::Stored => None,
        };
        let (method, payload) = match &compressed {
            Some(z) => (METHOD_DEFLATE, z.as_slice()),
            None => (METHOD_STORED, data),
        };
        let entry = Written {
            name: name.to_string(),
            method,
            flags: if name.is_ascii() { 0 } else { FLAG_UTF8 },
            crc: crc32(data),
            size: data.len() as u64,
            compressed_size: payload.len() as u64,
            offset: self.pos,
            time: self.dos_time.0,
            date: self.dos_time.1,
        };
        let header = self.local_header(&entry);
        self.emit(&header)?;
        self.emit(payload)?;
        self.entries.push(entry);
        Ok(())
    }

    fn needs_zip64(&self, v: u64) -> bool {
        self.force_zip64 || v >= U32_SENTINEL
    }

    fn local_header(&self, e: &Written) -> Vec<u8> {
        let big = self.needs_zip64(e.size) || self.needs_zip64(e.compressed_size);
        // The local ZIP64 field, when present, must hold both sizes.
        let extra =
            if big { Zip64Extra { size: Some(e.size), compressed_size: Some(e.compressed_size), local_header_offset: None }.encode() } else { Vec::new() };
        let mut h = Le(Vec::with_capacity(LOCAL_HEADER_LEN + e.name.len() + extra.len()));
        h.u32(LOCAL_HEADER_SIG)
            .u16(if big { VERSION_ZIP64 } else { VERSION_DEFLATE })
            .u16(e.flags)
            .u16(e.method)
            .u16(e.time)
            .u16(e.date)
            .u32(e.crc)
            .u32(if big { U32_SENTINEL as u32 } else { e.compressed_size as u32 })
            .u32(if big { U32_SENTINEL as u32 } else { e.size as u32 })
            .u16(e.name.len() as u16)
            .u16(extra.len() as u16)
            .bytes(e.name.as_bytes())
            .bytes(&extra);
        h.0
    }

    fn central_header(&self, e: &Written) -> Vec<u8> {
        let z = Zip64Extra {
            size: self.needs_zip64(e.size).then_some(e.size),
            compressed_size: self.needs_zip64(e.compressed_size).then_some(e.compressed_size),
            local_header_offset: self.needs_zip64(e.offset).then_some(e.offset),
        };
        let extra = z.encode();
        let version = if z.is_empty() { VERSION_DEFLATE } else { VERSION_ZIP64 };
        let is_dir = e.name.ends_with('/');
        // Unix mode in the high half of the external attributes; MS-DOS directory bit in the low byte.
        let attrs: u32 = if is_dir { (0o040755 << 16) | 0x10 } else { 0o100644 << 16 };
        let mut h = Le(Vec::with_capacity(CENTRAL_HEADER_LEN + e.name.len() + extra.len()));
        h.u32(CENTRAL_HEADER_SIG)
            .u16(MADE_BY_UNIX | version)
            .u16(version)
            .u16(e.flags)
            .u16(e.method)
            .u16(e.time)
            .u16(e.date)
            .u32(e.crc)
            .u32(z.compressed_size.map_or(e.compressed_size as u32, |_| U32_SENTINEL as u32))
            .u32(z.size.map_or(e.size as u32, |_| U32_SENTINEL as u32))
            .u16(e.name.len() as u16)
            .u16(extra.len() as u16)
            .u16(0) // comment length
            .u16(0) // disk number start
            .u16(0) // internal attributes
            .u32(attrs)
            .u32(z.local_header_offset.map_or(e.offset as u32, |_| U32_SENTINEL as u32))
            .bytes(e.name.as_bytes())
            .bytes(&extra);
        h.0
    }

    fn emit(&mut self, bytes: &[u8]) -> Result<(), ZipError> {
        self.out.write_all(bytes)?;
        self.pos += bytes.len() as u64;
        Ok(())
    }

    /// Write the central directory and end records, flush, and return the underlying writer.
    pub fn finish(mut self) -> Result<W, ZipError> {
        let cd_offset = self.pos;
        let headers: Vec<Vec<u8>> = self.entries.iter().map(|e| self.central_header(e)).collect();
        for h in &headers {
            self.emit(h)?;
        }
        let cd_size = self.pos - cd_offset;
        let count = self.entries.len() as u64;
        let zip64 = self.force_zip64 || count >= U16_SENTINEL || cd_size >= U32_SENTINEL || cd_offset >= U32_SENTINEL;
        if zip64 {
            let eocd64_offset = self.pos;
            let mut r = Le(Vec::with_capacity(ZIP64_EOCD_LEN + ZIP64_LOCATOR_LEN));
            r.u32(ZIP64_EOCD_SIG)
                .u64((ZIP64_EOCD_LEN - 12) as u64) // size of the rest of this record
                .u16(MADE_BY_UNIX | VERSION_ZIP64)
                .u16(VERSION_ZIP64)
                .u32(0) // this disk
                .u32(0) // disk with the central directory
                .u64(count)
                .u64(count)
                .u64(cd_size)
                .u64(cd_offset);
            r.u32(ZIP64_LOCATOR_SIG).u32(0).u64(eocd64_offset).u32(1);
            self.emit(&r.0)?;
        }
        // Values that do not fit are saturated to the sentinel; the ZIP64 record above then holds them.
        let mut r = Le(Vec::with_capacity(EOCD_LEN));
        let count16 = count.min(U16_SENTINEL) as u16;
        r.u32(EOCD_SIG).u16(0).u16(0).u16(count16).u16(count16).u32(cd_size.min(U32_SENTINEL) as u32).u32(cd_offset.min(U32_SENTINEL) as u32).u16(0);
        self.emit(&r.0)?;
        self.out.flush()?;
        Ok(self.out)
    }
}
