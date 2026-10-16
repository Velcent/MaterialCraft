//! Damaged archives give errors, never panics.

use super::fixtures::{full_document, Rng};
use super::round_trip::{from_bytes, to_bytes};
use crate::paint::format::{load_with, FormatError, LoadOptions};
use crate::zip::{Method, ZipReader, ZipWriter};
use std::io::Cursor;

/// The archive rebuilt with every entry passed through `edit` (returning `None` drops it).
pub fn rewrite(bytes: &[u8], mut edit: impl FnMut(&str, Vec<u8>) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut zip = ZipReader::open(Cursor::new(bytes)).unwrap();
    let mut out = ZipWriter::in_memory();
    for e in zip.entries().to_vec() {
        let data = zip.read_entry(&e).unwrap();
        if let Some(data) = edit(&e.name, data) {
            out.add(&e.name, &data, Method::Deflate).unwrap();
        }
    }
    out.finish_bytes().unwrap()
}

/// A small document, so that exhaustive corruption stays fast.
fn small() -> Vec<u8> {
    let mut doc = full_document();
    doc.texture_sets.truncate(1);
    let set = &mut doc.texture_sets[0];
    set.layers.truncate(4);
    set.baked.truncate(2);
    doc.foreign.truncate(1);
    doc.foreign[0].bytes.truncate(100);
    to_bytes(&doc)
}

#[test]
fn truncated_archives_error() {
    let bytes = small();
    for n in (0..bytes.len()).step_by(97).chain([bytes.len() - 1]) {
        assert!(from_bytes(&bytes[..n]).is_err(), "prefix {n}");
    }
}

#[test]
fn flipped_bytes_never_panic() {
    let bytes = small();
    let mut rng = Rng::new(11);
    let mut errors = 0;
    let rounds = 400;
    for _ in 0..rounds {
        let mut b = bytes.clone();
        for _ in 0..1 + rng.next() % 3 {
            let i = rng.next() as usize % b.len();
            b[i] ^= 1 << (rng.next() % 8);
        }
        if from_bytes(&b).is_err() {
            errors += 1;
        }
    }
    // Flips in unchecked header fields (timestamps, version-made-by) are harmless; most are caught.
    assert!(errors > rounds * 3 / 4, "{errors}/{rounds}");
}

#[test]
fn damaged_entries_error() {
    let bytes = small();
    let names: Vec<String> = ZipReader::open(Cursor::new(&bytes))
        .unwrap()
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .collect();
    for name in names.iter().filter(|n| *n != "thumbnail.png") {
        // Missing entry.
        let dropped = rewrite(&bytes, |n, d| (n != name).then_some(d));
        assert!(from_bytes(&dropped).is_err(), "without {name}");
        if name.starts_with("foreign/") {
            continue; // foreign bytes are opaque: any content is valid
        }
        // Valid ZIP, damaged content.
        let cut = rewrite(&bytes, |n, mut d| {
            if n == name {
                d.truncate(d.len() / 2);
            }
            Some(d)
        });
        assert!(from_bytes(&cut).is_err(), "truncated {name}");
        let mut rng = Rng::new(name.len() as u32);
        for _ in 0..20 {
            let flipped = rewrite(&bytes, |n, mut d| {
                if n == name && !d.is_empty() {
                    let i = rng.next() as usize % d.len();
                    d[i] ^= 0x10;
                }
                Some(d)
            });
            let _ = from_bytes(&flipped);
        }
    }
}

#[test]
fn image_budget_is_enforced() {
    let bytes = small();
    let tight = LoadOptions {
        max_image_bytes: 1000,
    };
    assert!(matches!(
        load_with(Cursor::new(&bytes), &tight),
        Err(FormatError::TooLarge { .. })
    ));
    assert!(load_with(Cursor::new(&bytes), &LoadOptions::default()).is_ok());
}

#[test]
fn not_a_document() {
    assert!(matches!(from_bytes(b"hello"), Err(FormatError::Zip(_))));
    let mut z = ZipWriter::in_memory();
    z.add("other.txt", b"x", Method::Stored).unwrap();
    let bytes = z.finish_bytes().unwrap();
    assert!(
        matches!(from_bytes(&bytes), Err(FormatError::MissingEntry(n)) if n == "document.json")
    );
}
