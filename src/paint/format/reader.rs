//! [`DocumentReader`]: reads the manifest, checks its version, then the mesh, images (decoded in
//! parallel batches), stroke records, foreign files and import report, and validates the result.

use super::batch::{batch_size, par_map};
use super::manifest::{self, ImageRef, report};
use super::{FormatError, images, paths, strokes};
use crate::json;
use crate::mesh::Mesh;
use crate::paint::Document;
use crate::zip::ZipReader;
use std::io::{Read, Seek};

/// Limits for [`super::load_with`].
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// Most bytes of decoded image data a document may declare (default 16 GiB). Checked
    /// against the manifest before any image is decoded.
    pub max_image_bytes: u64,
}

impl Default for LoadOptions {
    fn default() -> LoadOptions {
        LoadOptions { max_image_bytes: 16 << 30 }
    }
}

/// Reads one document from a ZIP archive.
pub struct DocumentReader<R: Read + Seek> {
    zip: ZipReader<R>,
    options: LoadOptions,
}

impl<R: Read + Seek> DocumentReader<R> {
    /// Opens the archive (reads its central directory only).
    pub fn open(input: R, options: &LoadOptions) -> Result<DocumentReader<R>, FormatError> {
        Ok(DocumentReader { zip: ZipReader::open(input)?, options: options.clone() })
    }

    pub fn read(mut self) -> Result<Document, FormatError> {
        let root = self.json(paths::DOCUMENT)?;
        let m = manifest::read(&root)?;
        drop(root);
        let total = m.images.iter().fold(0u64, |sum, r| {
            let bytes = r.size.0 as u64 * r.size.1 as u64 * r.format.bytes_per_pixel() as u64;
            sum.saturating_add(bytes)
        });
        if total > self.options.max_image_bytes {
            return Err(FormatError::TooLarge { limit: self.options.max_image_bytes });
        }
        let mut doc = m.doc;
        doc.mesh = Mesh::from_glb(&self.zip.read(&m.mesh)?)?;
        // The skeleton already has every image's size and format: reject a broken document
        // before decoding any pixels.
        doc.validate()?;
        self.read_images(&mut doc, &m.images)?;
        for r in &m.strokes {
            // A record that cannot be read is kept as unreadable; it never stops the load.
            let data = strokes::read_entry(&mut self.zip, &r.path);
            if let Some(slot) = r.slot.strokes_mut(&mut doc) {
                *slot = Some(data);
            }
        }
        for path in m.foreign.iter().filter(|p| *p != manifest::extras::Extras::PATH) {
            paths::check_foreign_path(path)?;
            let bytes = self.zip.read(path)?;
            doc.foreign.push(crate::paint::ForeignFile { path: path.to_string(), bytes });
        }
        doc.foreign.extend(m.extras.to_foreign());
        if let Some(path) = &m.import_report {
            doc.import_report = Some(report::read(&self.json(path)?)?);
        }
        doc.validate()?;
        Ok(doc)
    }

    fn json(&mut self, entry: &str) -> Result<json::Value, FormatError> {
        let bytes = self.zip.read(entry)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| FormatError::Json(format!("{entry} is not UTF-8")))?;
        json::parse(text).map_err(|e| FormatError::Json(format!("{entry}: {e}")))
    }

    /// Reads a batch of entries, decodes them in parallel and moves them into place.
    fn read_images(&mut self, doc: &mut Document, refs: &[ImageRef]) -> Result<(), FormatError> {
        for batch in refs.chunks(batch_size()) {
            let entries = batch.iter().map(|r| Ok((r, self.zip.read(&r.path)?))).collect::<Result<Vec<_>, FormatError>>()?;
            let decoded = par_map(&entries, |(r, bytes): &(&ImageRef, Vec<u8>)| images::decode(bytes, r.format, r.size));
            drop(entries);
            for (r, img) in batch.iter().zip(decoded) {
                let img = img.map_err(|m| FormatError::image(&r.path, m))?;
                let slot = r.slot.image_mut(doc).ok_or_else(|| FormatError::image(&r.path, "no image of the document refers to it"))?;
                *slot = img;
            }
        }
        Ok(())
    }
}
