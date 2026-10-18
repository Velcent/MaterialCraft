//! The stroke record entry (`*.mcstroke`): a small binary layout, little-endian, written and read exactly.
//!
//! ```text
//! "MCSTROKE"  u16 major (1)  u16 minor (0, 1 or 2)  u32 stroke count
//! per stroke: u8 tool (0 paint, 1 eraser, 2 smudge, 3 projection, 4 clone, 5 blur)  u8 view  u16 preset length
//!             preset (UTF-8)
//!             u32 seed  u32 symmetry copies  [1.1+: u8 symmetry flips, bit 0 = U, bit 1 = V]
//!             f32×16 symmetry transform  f32×16 view  f32×16 projection
//!             u32 sample count, then six columns of that many f32: x, y, pressure, tilt x, tilt y, time
//!             [1.2: a seventh column, pen rotation]
//! ```
//!
//! Minor 1 adds the symmetry flips (radial copies flipped for lettering, roadmap step 30). It is written only when a
//! stroke has a flip, so records without one keep the 1.0 layout byte for byte; a 1.1 entry without any flip is
//! not one this writer makes and is refused.
//!
//! Minor 2 adds the pen's barrel rotation (tablet input, roadmap step 35) and keeps 1.1's flips byte. It is written
//! only when a sample has a rotation (any bits set), so mouse and imported strokes keep the older layouts; a 1.2
//! entry without any rotation is refused for the same reason.
//!
//! Tool codes 4 (clone) and 5 (blur) came with document format 0.11 (roadmap step 36); an older reader keeps such an
//! entry as unreadable bytes.
//!
//! Decoding is strict (known codes, valid UTF-8, no bytes left over), so every readable entry encodes back to
//! the same bytes. Floats keep their bits.

use crate::paint::stroke::{Camera, Mat4, Stroke, StrokeRecord, StrokeSample, StrokeTool, Symmetry, ViewKind};

pub const MAGIC: &[u8; 8] = b"MCSTROKE";
pub const MAJOR: u16 = 1;
pub const MINOR: u16 = 2;
/// Extension of stroke record entries.
pub const EXTENSION: &str = "mcstroke";

/// Bytes of a stroke without its preset and samples.
const STROKE_FIXED: usize = 1 + 1 + 2 + 4 + 4 + 3 * 64 + 4;
/// Bytes per sample (six f32; seven from 1.2).
const SAMPLE_BYTES: usize = 24;

/// Does any sample of `record` carry a rotation (bits, so -0.0 counts and round-trips)?
fn has_rotation(record: &StrokeRecord) -> bool {
    record.strokes.iter().any(|s| s.samples.iter().any(|p| p.rotation.to_bits() != 0))
}

fn tool_code(t: StrokeTool) -> u8 {
    match t {
        StrokeTool::Paint => 0,
        StrokeTool::Eraser => 1,
        StrokeTool::Smudge => 2,
        StrokeTool::Projection => 3,
        StrokeTool::Clone => 4,
        StrokeTool::Blur => 5,
    }
}

fn view_code(v: ViewKind) -> u8 {
    match v {
        ViewKind::Uv => 0,
        ViewKind::Mesh => 1,
    }
}

/// The entry bytes of `record`. Fails only for sizes the layout cannot hold (a preset over 65535 bytes, more
/// than 2³² − 1 strokes or samples).
pub fn encode(record: &StrokeRecord) -> Result<Vec<u8>, String> {
    let samples = record.sample_count();
    let mut out = Vec::with_capacity(16 + record.strokes.len() * STROKE_FIXED + samples * SAMPLE_BYTES);
    out.extend_from_slice(MAGIC);
    let rotation = has_rotation(record);
    let flips = rotation || record.strokes.iter().any(|s| s.symmetry.flip != [false; 2]);
    let minor: u16 = if rotation { 2 } else { u16::from(flips) };
    out.extend_from_slice(&MAJOR.to_le_bytes());
    out.extend_from_slice(&minor.to_le_bytes());
    out.extend_from_slice(&count(record.strokes.len(), "strokes")?.to_le_bytes());
    for s in &record.strokes {
        out.push(tool_code(s.tool));
        out.push(view_code(s.view));
        let preset = u16::try_from(s.preset.len()).map_err(|_| format!("brush preset reference of {} bytes is too long", s.preset.len()))?;
        out.extend_from_slice(&preset.to_le_bytes());
        out.extend_from_slice(s.preset.as_bytes());
        out.extend_from_slice(&s.seed.to_le_bytes());
        out.extend_from_slice(&s.symmetry.copies.to_le_bytes());
        if flips {
            out.push(u8::from(s.symmetry.flip[0]) | (u8::from(s.symmetry.flip[1]) << 1));
        }
        for m in [&s.symmetry.transform, &s.camera.view, &s.camera.projection] {
            m.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
        }
        out.extend_from_slice(&count(s.samples.len(), "samples")?.to_le_bytes());
        let columns: [fn(&StrokeSample) -> f32; 6] = [|p| p.position[0], |p| p.position[1], |p| p.pressure, |p| p.tilt[0], |p| p.tilt[1], |p| p.time];
        for column in columns {
            s.samples.iter().for_each(|p| out.extend_from_slice(&column(p).to_le_bytes()));
        }
        if rotation {
            s.samples.iter().for_each(|p| out.extend_from_slice(&p.rotation.to_le_bytes()));
        }
    }
    Ok(out)
}

fn count(n: usize, what: &str) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| format!("{n} {what} are more than a stroke record holds"))
}

/// Reads an entry. The error says why it is not a stroke record this version can read.
pub fn decode(bytes: &[u8]) -> Result<StrokeRecord, String> {
    let mut r = Reader { bytes, pos: 0 };
    if r.take(MAGIC.len())? != MAGIC {
        return Err("not a stroke record".into());
    }
    let (major, minor) = (r.u16()?, r.u16()?);
    if major != MAJOR || minor > MINOR {
        return Err(format!("stroke record version {major}.{minor}; this program reads {MAJOR}.0 … {MAJOR}.{MINOR}"));
    }
    let (flips, rotation) = (minor >= 1, minor >= 2);
    let n = r.u32()? as usize;
    r.fits(n, STROKE_FIXED)?;
    let mut strokes = Vec::with_capacity(n);
    for _ in 0..n {
        strokes.push(r.stroke(flips, rotation)?);
    }
    if minor == 1 && strokes.iter().all(|s| s.symmetry.flip == [false; 2]) {
        return Err("stroke record 1.1 without symmetry flips".into());
    }
    let record = StrokeRecord { strokes };
    if rotation && !has_rotation(&record) {
        return Err("stroke record 1.2 without pen rotation".into());
    }
    if r.pos != bytes.len() {
        return Err(format!("{} bytes after the last stroke", bytes.len() - r.pos));
    }
    Ok(record)
}

/// A cursor over the entry; every read checks the length.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or("stroke record is cut short")?;
        let out = self.bytes.get(self.pos..end).ok_or("stroke record is cut short")?;
        self.pos = end;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    fn mat4(&mut self) -> Result<Mat4, String> {
        let mut m = [0.0; 16];
        for v in &mut m {
            *v = self.f32()?;
        }
        Ok(m)
    }

    /// `n` items of at least `size` bytes fit in what is left (checked before allocating for them).
    fn fits(&self, n: usize, size: usize) -> Result<(), String> {
        match n.checked_mul(size) {
            Some(b) if b <= self.bytes.len() - self.pos.min(self.bytes.len()) => Ok(()),
            _ => Err(format!("{n} entries do not fit in the stroke record")),
        }
    }

    fn stroke(&mut self, flips: bool, rotation: bool) -> Result<Stroke, String> {
        let tool = match self.u8()? {
            0 => StrokeTool::Paint,
            1 => StrokeTool::Eraser,
            2 => StrokeTool::Smudge,
            3 => StrokeTool::Projection,
            4 => StrokeTool::Clone,
            5 => StrokeTool::Blur,
            other => return Err(format!("unknown tool code {other}")),
        };
        let view = match self.u8()? {
            0 => ViewKind::Uv,
            1 => ViewKind::Mesh,
            other => return Err(format!("unknown view code {other}")),
        };
        let len = self.u16()? as usize;
        let preset = std::str::from_utf8(self.take(len)?).map_err(|_| "brush preset reference is not UTF-8")?.to_string();
        let seed = self.u32()?;
        let copies = self.u32()?;
        let flip = match if flips { self.u8()? } else { 0 } {
            f @ 0..=3 => [f & 1 != 0, f & 2 != 0],
            other => return Err(format!("unknown symmetry flips {other}")),
        };
        let symmetry = Symmetry { copies, transform: self.mat4()?, flip };
        let camera = Camera { view: self.mat4()?, projection: self.mat4()? };
        let n = self.u32()? as usize;
        self.fits(n, SAMPLE_BYTES + if rotation { 4 } else { 0 })?;
        let mut columns: [Vec<f32>; 7] = Default::default();
        let read = if rotation { 7 } else { 6 };
        for column in columns.iter_mut().take(read) {
            column.reserve_exact(n);
            for _ in 0..n {
                column.push(self.f32()?);
            }
        }
        let [x, y, pressure, tilt_x, tilt_y, time, rotation] = &columns;
        let samples = (0..n)
            .map(|i| {
                let at = |c: &Vec<f32>| c.get(i).copied().unwrap_or(0.0);
                StrokeSample { position: [at(x), at(y)], pressure: at(pressure), tilt: [at(tilt_x), at(tilt_y)], rotation: at(rotation), time: at(time) }
            })
            .collect();
        Ok(Stroke { tool, preset, view, camera, seed, symmetry, samples })
    }
}
