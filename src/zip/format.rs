//! On-disk records of the ZIP format (APPNOTE 6.3): signatures, limits, little-endian field access and the
//! ZIP64 extended-information extra field.

pub(super) const LOCAL_HEADER_SIG: u32 = 0x0403_4b50;
pub(super) const CENTRAL_HEADER_SIG: u32 = 0x0201_4b50;
pub(super) const EOCD_SIG: u32 = 0x0605_4b50;
pub(super) const ZIP64_EOCD_SIG: u32 = 0x0606_4b50;
pub(super) const ZIP64_LOCATOR_SIG: u32 = 0x0706_4b50;

pub(super) const LOCAL_HEADER_LEN: usize = 30;
pub(super) const CENTRAL_HEADER_LEN: usize = 46;
pub(super) const EOCD_LEN: usize = 22;
pub(super) const ZIP64_EOCD_LEN: usize = 56;
pub(super) const ZIP64_LOCATOR_LEN: usize = 20;

/// Extra-field tag of the ZIP64 extended information.
pub(super) const ZIP64_EXTRA_TAG: u16 = 0x0001;

/// 32-bit size/offset fields hold this when the real value lives in the ZIP64 extra field.
pub(super) const U32_SENTINEL: u64 = 0xffff_ffff;
/// 16-bit entry counts hold this when the real count lives in the ZIP64 end record.
pub(super) const U16_SENTINEL: u64 = 0xffff;

/// General-purpose flag bits.
pub(super) const FLAG_ENCRYPTED: u16 = 1 << 0;
pub(super) const FLAG_UTF8: u16 = 1 << 11;

/// "Version made by"/"needed to extract": 2.0 for deflate, 4.5 for ZIP64. The high byte 3 marks Unix attributes.
pub(super) const VERSION_DEFLATE: u16 = 20;
pub(super) const VERSION_ZIP64: u16 = 45;
pub(super) const MADE_BY_UNIX: u16 = 3 << 8;

/// Compression method codes.
pub(super) const METHOD_STORED: u16 = 0;
pub(super) const METHOD_DEFLATE: u16 = 8;

/// Little-endian appender for building records.
pub(super) struct Le(pub Vec<u8>);

impl Le {
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }
}

/// Bounds-checked little-endian cursor over a record; every read fails cleanly at the end of the data.
pub(super) struct Fields<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Fields<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Fields { data, pos: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(s)
    }

    pub fn u16(&mut self) -> Option<u16> {
        self.bytes(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn u32(&mut self) -> Option<u32> {
        self.bytes(4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn u64(&mut self) -> Option<u64> {
        self.bytes(8)
            .map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
    }
}

/// Values a ZIP64 extra field may carry, in their mandated order. Only fields whose 32-bit header slot holds the
/// sentinel are present.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Zip64Extra {
    pub size: Option<u64>,
    pub compressed_size: Option<u64>,
    pub local_header_offset: Option<u64>,
}

impl Zip64Extra {
    pub fn is_empty(&self) -> bool {
        self.size.is_none() && self.compressed_size.is_none() && self.local_header_offset.is_none()
    }

    /// The whole extra field (tag, length, payload), or nothing when no value needs it.
    pub fn encode(&self) -> Vec<u8> {
        if self.is_empty() {
            return Vec::new();
        }
        let mut payload = Le(Vec::new());
        for v in [self.size, self.compressed_size, self.local_header_offset]
            .into_iter()
            .flatten()
        {
            payload.u64(v);
        }
        let mut out = Le(Vec::new());
        out.u16(ZIP64_EXTRA_TAG)
            .u16(payload.0.len() as u16)
            .bytes(&payload.0);
        out.0
    }

    /// Fill in the sentinel-valued fields from the ZIP64 record inside an extra-field block.
    pub fn decode(
        extra: &[u8],
        need_size: bool,
        need_compressed: bool,
        need_offset: bool,
    ) -> Option<Zip64Extra> {
        let mut f = Fields::new(extra);
        while let (Some(tag), Some(len)) = (f.u16(), f.u16()) {
            let body = f.bytes(len as usize)?;
            if tag != ZIP64_EXTRA_TAG {
                continue;
            }
            let mut b = Fields::new(body);
            let mut z = Zip64Extra::default();
            if need_size {
                z.size = Some(b.u64()?);
            }
            if need_compressed {
                z.compressed_size = Some(b.u64()?);
            }
            if need_offset {
                z.local_header_offset = Some(b.u64()?);
            }
            return Some(z);
        }
        None
    }
}

/// MS-DOS date and time (2-second resolution, 1980-2107) of a Unix timestamp, in UTC.
pub(super) fn dos_datetime(unix_secs: i64) -> (u16, u16) {
    let days = unix_secs.div_euclid(86_400);
    let secs = unix_secs.rem_euclid(86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    if year < 1980 {
        return (0, (1 << 5) | 1);
    }
    let year = year.min(2107);
    let time = ((secs / 3600) << 11) | (((secs / 60) % 60) << 5) | ((secs % 60) / 2);
    let date = ((year - 1980) << 9) | (month << 5) | day;
    (time as u16, date as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_dates() {
        assert_eq!(dos_datetime(0), (0, 0x21));
        // 2024-02-29 13:45:30 UTC
        let (t, d) = dos_datetime(1_709_214_330);
        assert_eq!((d >> 9, (d >> 5) & 15, d & 31), (44, 2, 29));
        assert_eq!((t >> 11, (t >> 5) & 63, (t & 31) * 2), (13, 45, 30));
    }

    #[test]
    fn zip64_extra_round_trip() {
        let z = Zip64Extra {
            size: Some(5 << 32),
            compressed_size: None,
            local_header_offset: Some(7),
        };
        let mut extra = vec![0x55, 0x54, 1, 0, 9];
        extra.extend_from_slice(&z.encode());
        assert_eq!(Zip64Extra::decode(&extra, true, false, true), Some(z));
        assert_eq!(Zip64Extra::decode(&extra, true, true, true), None);
        assert!(Zip64Extra::default().encode().is_empty());
    }
}
