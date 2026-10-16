use super::*;
use crate::paint::{PixelLayout, SampleType};

fn noisy(format: PixelFormat, w: u32, h: u32) -> TiledImage {
    let n = (w * h) as usize * format.bytes_per_pixel();
    let mut s = 7u32;
    let mut raw: Vec<u8> = (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (s >> 16) as u8
        })
        .collect();
    if format.sample == SampleType::F32 {
        // Keep floats finite (NaN payloads are not part of the contract).
        raw.chunks_exact_mut(4).for_each(|c| c[3] &= 0x3f);
    }
    if format.sample == SampleType::F16 {
        raw.chunks_exact_mut(2).for_each(|c| c[1] &= 0x3b);
    }
    TiledImage::from_raw(w, h, format, &raw).unwrap()
}

#[test]
fn every_format_round_trips_bit_exactly() {
    for layout in PixelLayout::ALL {
        for sample in SampleType::ALL {
            let f = PixelFormat::new(layout, sample);
            let img = noisy(f, 37, 130);
            let bytes = encode(&img).unwrap();
            let back = decode(&bytes, f, (37, 130)).unwrap();
            assert_eq!(back.to_raw(), img.to_raw(), "{}", f.id());
            // A mismatching declaration is an error, not a conversion.
            let other = PixelFormat::new(layout.with_alpha(), sample);
            if other != f {
                assert!(decode(&bytes, other, (37, 130)).is_err());
            }
            assert!(decode(&bytes, f, (36, 130)).is_err());
        }
    }
}

#[test]
fn corrupt_entries_error() {
    for id in ["rgba16", "rgb16f"] {
        let f = PixelFormat::from_id(id).unwrap();
        let bytes = encode(&noisy(f, 20, 9)).unwrap();
        // A PNG cut inside its final IEND chunk still holds all pixels; the ZIP CRC catches that.
        let end = if id == "rgba16" {
            bytes.len() - 12
        } else {
            bytes.len()
        };
        for n in (0..end).step_by(5) {
            assert!(decode(&bytes[..n], f, (20, 9)).is_err(), "{id} prefix {n}");
        }
        let mut s = 3u32;
        for _ in 0..300 {
            let mut b = bytes.clone();
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let i = (s >> 8) as usize % b.len();
            b[i] ^= 0x41;
            let _ = decode(&b, f, (20, 9));
        }
    }
}
