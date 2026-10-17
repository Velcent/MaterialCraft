//! Embedded assets (since 0.6): each [`EmbeddedAsset`] is the entry `resources/assets/<reference>`, deflated; the
//! manifest's `assets` array lists the references in order. Shared by documents and fragments.

use super::FormatError;
use crate::json::Value;
use crate::paint::embedded::{EmbeddedAsset, is_embeddable};
use crate::zip::{Method, ZipReader, ZipWriter};
use std::io::{Read, Seek, Write};

/// Folder of embedded asset entries.
pub const PREFIX: &str = "resources/assets/";

/// The entry holding the asset embedded under `reference`.
pub fn entry(reference: &str) -> String {
    format!("{PREFIX}{reference}")
}

/// Checks that every reference can be an entry name and appears once.
pub fn check(assets: &[EmbeddedAsset]) -> Result<(), FormatError> {
    for (i, a) in assets.iter().enumerate() {
        check_reference(&a.reference)?;
        if assets[..i].iter().any(|b| b.reference == a.reference) {
            return Err(FormatError::Unsupported(format!("asset {:?} is embedded twice", a.reference)));
        }
    }
    Ok(())
}

fn check_reference(reference: &str) -> Result<(), FormatError> {
    if is_embeddable(reference) {
        Ok(())
    } else {
        Err(FormatError::Unsupported(format!("embedded asset {reference:?} must be a relative library path like \"materials/rust.mca\"")))
    }
}

/// The manifest's `assets` array.
pub fn manifest(assets: &[EmbeddedAsset]) -> Value {
    assets.iter().map(|a| a.reference.as_str()).collect()
}

/// The references of a manifest's `assets` array (`at`: its JSON path).
pub fn references(list: &[Value], at: &str) -> Result<Vec<String>, FormatError> {
    list.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| FormatError::manifest(at, "expected an array of asset references"))).collect()
}

/// Writes every asset's entry.
pub fn write<W: Write>(assets: &[EmbeddedAsset], zip: &mut ZipWriter<W>) -> Result<(), FormatError> {
    for a in assets {
        zip.add(&entry(&a.reference), &a.bytes, Method::Deflate)?;
    }
    Ok(())
}

/// Reads the assets embedded under `references`.
pub fn read<R: Read + Seek>(references: &[String], zip: &mut ZipReader<R>) -> Result<Vec<EmbeddedAsset>, FormatError> {
    let mut out: Vec<EmbeddedAsset> = Vec::with_capacity(references.len());
    for r in references {
        check_reference(r)?;
        if out.iter().any(|a| &a.reference == r) {
            return Err(FormatError::Unsupported(format!("asset {r:?} is embedded twice")));
        }
        out.push(EmbeddedAsset::new(r.clone(), zip.read(&entry(r))?));
    }
    Ok(out)
}
