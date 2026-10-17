//! `.mcfrag` files: smart materials and smart masks ([`Fragment`]). A fragment is a `.mcpaint`-style ZIP archive
//! without a mesh: a JSON manifest (`fragment.json`, see [`manifest`]), the images and stroke records of its
//! layers or mask exactly as a document stores them, its embedded assets under `resources/assets/`, and an
//! optional `thumbnail.png`. The specification is the "Fragments" section of `formats/mcpaint.md` in the
//! MaterialCraft docs.
//!
//! ```
//! use materialcraft::paint::format::fragment;
//! use materialcraft::paint::{Fragment, Mask, TextureSet};
//! let f = Fragment::smart_mask("Edges", &TextureSet::new("s", "S", (64, 64)), Mask::new(0.0));
//! let mut bytes = Vec::new();
//! fragment::save(&f, &mut bytes).unwrap();
//! assert_eq!(fragment::load(std::io::Cursor::new(bytes)).unwrap(), f);
//! ```

pub mod manifest;

pub use manifest::{FRAGMENT_FORMAT, FragmentInfo};

use super::manifest::extras::Extras;
use super::manifest::{ReadCtx, WriteCtx, fields::Obj};
use super::paths::{ImagePlan, THUMBNAIL};
use super::reader::{check_budget, read_images, read_json, read_strokes};
use super::strokes::StrokePlan;
use super::writer::{json_bytes, save_atomically, stroke_entries, write_images};
use super::{FormatError, LoadOptions, SaveOptions, embedded};
use crate::paint::{Document, Fragment};
use crate::zip::{Method, ZipReader, ZipWriter};
use std::io::{Read, Seek, Write};
use std::path::Path;

/// File extension of fragments (without the dot).
pub const EXTENSION: &str = "mcfrag";
/// The manifest entry.
pub const MANIFEST: &str = "fragment.json";

/// Writes `f` with default options (a thumbnail of a smart material's base colour). Same fragment, same bytes.
pub fn save(f: &Fragment, out: impl Write) -> Result<(), FormatError> {
    save_with(f, out, &SaveOptions::default())
}

/// [`save`] with explicit options.
pub fn save_with(f: &Fragment, out: impl Write, options: &SaveOptions) -> Result<(), FormatError> {
    f.validate()?;
    embedded::check(&f.assets)?;
    // The carrier document is what the document writer's pieces work on.
    let doc = Document { texture_sets: vec![f.carrier()], ..Document::default() };
    let set = doc.texture_sets.first().ok_or_else(|| FormatError::Unsupported("fragment without content".into()))?;
    let plan = ImagePlan::new(&doc);
    let strokes = StrokePlan::new(&doc, &plan);
    let stroke_bytes = stroke_entries(&strokes)?;
    let extras = Extras::default();
    let thumbnail = options.thumbnail.render(&doc);
    let ctx = WriteCtx { plan: &plan, strokes: &strokes, extras: &extras };
    let json = manifest::write(f, set, &ctx, thumbnail.as_ref().map(|_| THUMBNAIL));

    let mut zip = ZipWriter::new(out);
    zip.add(MANIFEST, &json_bytes(&json), Method::Deflate)?;
    write_images(&plan, &mut zip)?;
    for (path, bytes) in stroke_bytes {
        zip.add(path, &bytes, Method::Deflate)?;
    }
    embedded::write(&f.assets, &mut zip)?;
    if let Some(png) = thumbnail {
        zip.add(THUMBNAIL, &png, Method::Stored)?;
    }
    zip.finish()?.flush()?;
    Ok(())
}

/// Saves to `path` through a temporary file renamed over it once complete.
pub fn save_path(f: &Fragment, path: &Path) -> Result<(), FormatError> {
    save_atomically(path, |out| save(f, out))
}

/// [`save_path`] with explicit options (e.g. a thumbnail of the viewport).
pub fn save_path_with(f: &Fragment, path: &Path, options: &SaveOptions) -> Result<(), FormatError> {
    save_atomically(path, |out| save_with(f, out, options))
}

/// Reads a fragment with default limits.
pub fn load(input: impl Read + Seek) -> Result<Fragment, FormatError> {
    load_with(input, &LoadOptions::default())
}

/// [`load`] with explicit limits.
pub fn load_with(input: impl Read + Seek, options: &LoadOptions) -> Result<Fragment, FormatError> {
    let mut zip = ZipReader::open(input)?;
    let root = read_json(&mut zip, MANIFEST)?;
    let mut ctx = ReadCtx::default();
    let read = manifest::read(&root, &mut ctx)?;
    drop(root);
    check_budget(&ctx.images, options)?;
    let mut doc = Document { texture_sets: vec![read.set], ..Document::default() };
    doc.validate()?;
    read_images(&mut zip, &mut doc, &ctx.images)?;
    read_strokes(&mut zip, &mut doc, &ctx.strokes);
    let assets = embedded::read(&read.assets, &mut zip)?;
    let set = doc.texture_sets.pop().ok_or_else(|| FormatError::Unsupported("fragment without content".into()))?;
    let fragment = Fragment::from_carrier(read.info.kind, set, read.info.needs, assets)
        .ok_or_else(|| FormatError::manifest("mask", "a smart mask holds exactly one mask"))?;
    fragment.validate()?;
    Ok(fragment)
}

/// Reads the fragment at `path`.
pub fn load_path(path: &Path) -> Result<Fragment, FormatError> {
    load(std::io::BufReader::new(std::fs::File::open(path)?))
}

/// The header of a fragment (kind, name, needed maps, thumbnail), reading only its manifest.
pub fn read_info(input: impl Read + Seek) -> Result<FragmentInfo, FormatError> {
    let mut zip = ZipReader::open(input)?;
    let root = read_json(&mut zip, MANIFEST)?;
    manifest::read_info(&mut Obj::new(&root, "")?)
}

/// [`read_info`] of the file at `path`.
pub fn read_info_path(path: &Path) -> Result<FragmentInfo, FormatError> {
    read_info(std::io::BufReader::new(std::fs::File::open(path)?))
}

/// The thumbnail PNG of the fragment at `path`, if it has one.
pub fn read_thumbnail_path(path: &Path) -> Result<Option<Vec<u8>>, FormatError> {
    let mut zip = ZipReader::open(std::io::BufReader::new(std::fs::File::open(path)?))?;
    let root = read_json(&mut zip, MANIFEST)?;
    let info = manifest::read_info(&mut Obj::new(&root, "")?)?;
    match info.thumbnail {
        Some(entry) => Ok(Some(zip.read(&entry)?)),
        None => Ok(None),
    }
}
