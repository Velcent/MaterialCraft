use super::codec::{self, decode, encode};
use crate::paint::stroke::{Camera, IDENTITY, Stroke, StrokeRecord, StrokeSample, StrokeTool, Symmetry, ViewKind};

/// A record with every field set to something that is not a default, including odd float bit patterns.
pub fn sample_record(seed: u32) -> StrokeRecord {
    let f = |i: u32| (i.wrapping_mul(2654435761).wrapping_add(seed) % 10_000) as f32 / 977.0 - 3.0;
    let mut mirror = IDENTITY;
    mirror[0] = -1.0;
    let stroke = |n: u32, tool, view, preset: &str| Stroke {
        tool,
        preset: preset.into(),
        view,
        camera: Camera { view: std::array::from_fn(|i| f(i as u32 + n)), projection: std::array::from_fn(|i| f(i as u32 * 7 + n)) },
        seed: seed.wrapping_mul(n + 1),
        symmetry: Symmetry { copies: n % 3, transform: mirror, flip: [false; 2] },
        samples: (0..n * 5)
            .map(|i| StrokeSample { position: [f(i), f(i + 1)], pressure: f(i + 2).abs() / 4.0, tilt: [f(i + 3), -0.0], time: i as f32 * 0.016 })
            .collect(),
    };
    StrokeRecord {
        strokes: vec![
            stroke(1, StrokeTool::Paint, ViewKind::Mesh, "substance:DataBrush/2978"),
            stroke(0, StrokeTool::Eraser, ViewKind::Uv, ""),
            stroke(7, StrokeTool::Smudge, ViewKind::Mesh, "pinsel/größe"),
            stroke(3, StrokeTool::Projection, ViewKind::Uv, "x"),
        ],
    }
}

#[test]
fn records_round_trip_byte_identically() {
    for record in [StrokeRecord::default(), sample_record(1), sample_record(99)] {
        let bytes = encode(&record).unwrap();
        let back = decode(&bytes).unwrap();
        assert_eq!(back, record);
        assert_eq!(encode(&back).unwrap(), bytes);
    }
}

#[test]
fn float_bits_are_kept() {
    let mut record = sample_record(5);
    let odd = [f32::NAN, f32::from_bits(0x7fc0_1234), f32::INFINITY, -0.0, f32::MIN_POSITIVE / 2.0];
    for (i, v) in odd.into_iter().enumerate() {
        record.strokes[0].samples[i].pressure = v;
    }
    let bytes = encode(&record).unwrap();
    let back = decode(&bytes).unwrap();
    let bits = |r: &StrokeRecord| r.strokes[0].samples.iter().map(|s| s.pressure.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&back), bits(&record));
    assert_eq!(encode(&back).unwrap(), bytes);
}

#[test]
fn layout_is_as_specified() {
    let record = StrokeRecord { strokes: vec![Stroke { samples: vec![StrokeSample::at([0.5, 0.25], 2.0)], ..sample_record(3).strokes[3].clone() }] };
    let bytes = encode(&record).unwrap();
    assert_eq!(&bytes[..8], codec::MAGIC);
    assert_eq!(&bytes[8..16], &[1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&bytes[16..20], &[3, 0, 1, 0], "projection tool, UV view, preset of 1 byte");
    assert_eq!(bytes.len(), 16 + 4 + 1 + 4 + 4 + 3 * 64 + 4 + 24);
    let columns = &bytes[bytes.len() - 24..];
    let col = |i: usize| f32::from_le_bytes(columns[i * 4..i * 4 + 4].try_into().unwrap());
    assert_eq!([col(0), col(1), col(2), col(3), col(4), col(5)], [0.5, 0.25, 1.0, 0.0, 0.0, 2.0]);
}

#[test]
fn damaged_entries_are_errors_not_panics() {
    let bytes = encode(&sample_record(7)).unwrap();
    for n in 0..bytes.len() {
        assert!(decode(&bytes[..n]).is_err(), "prefix {n}");
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(decode(&longer).unwrap_err().contains("after the last stroke"));
    let edit = |at: usize, v: u8| {
        let mut b = bytes.clone();
        b[at] = v;
        decode(&b)
    };
    assert!(edit(0, b'X').unwrap_err().contains("not a stroke record"));
    assert!(edit(8, 2).unwrap_err().contains("version 2.0"));
    assert!(edit(10, 2).unwrap_err().contains("version 1.2"));
    assert!(edit(16, 9).unwrap_err().contains("tool"));
    assert!(edit(17, 9).unwrap_err().contains("view"));
    assert!(edit(15, 0xff).is_err(), "a stroke count that cannot fit");
    // Every single-byte change either fails or reads back to exactly the changed bytes.
    for at in (0..bytes.len()).step_by(13) {
        let mut b = bytes.clone();
        b[at] ^= 0x5a;
        if let Ok(r) = decode(&b) {
            assert_eq!(encode(&r).unwrap(), b, "byte {at}");
        }
    }
}

#[test]
fn oversized_presets_cannot_be_encoded() {
    let mut record = sample_record(2);
    record.strokes[0].preset = "p".repeat(70_000);
    assert!(encode(&record).unwrap_err().contains("too long"));
}

#[test]
fn symmetry_flips_use_minor_one_and_older_records_keep_their_bytes() {
    let plain = sample_record(4);
    let bytes = encode(&plain).unwrap();
    assert_eq!(&bytes[10..12], &[0, 0], "no flip: still 1.0");
    let mut flipped = plain.clone();
    flipped.strokes[2].symmetry.flip = [false, true];
    let fb = encode(&flipped).unwrap();
    assert_eq!(&fb[10..12], &[1, 0]);
    assert_eq!(fb.len(), bytes.len() + flipped.strokes.len());
    let back = decode(&fb).unwrap();
    assert_eq!(back, flipped);
    assert_eq!(encode(&back).unwrap(), fb);
    // a 1.1 entry whose strokes have no flip is not canonical
    let mut odd = bytes.clone();
    odd[10] = 1;
    assert!(decode(&odd).is_err());
    let mut newer = bytes;
    newer[10] = 2;
    assert!(decode(&newer).unwrap_err().contains("version 1.2"));
}
