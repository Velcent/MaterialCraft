//! [`DocumentWriter`]: validates a document and writes its archive entries in a fixed order:
//! `document.json`, `import-report.json`, `mesh/model.glb`, images, stroke records, foreign files,
//! `thumbnail.png`. JSON, glTF and stroke records are deflated; PNG and EXR are stored (already compressed).

use super::batch::{batch_size, par_map};
use super::manifest::{self, WriteCtx, extras::Extras, report};
use super::paths::{self, ImagePlan};
use super::strokes::{self, StrokePlan};
use super::{FormatError, Thumbnail, images};
use crate::deflate::Level;
use crate::paint::Document;
use crate::zip::{Method, ZipWriter};
use std::io::Write;
use std::path::Path;

/// Settings for [`super::save_with`].
#[derive(Debug, Clone, Default)]
pub struct SaveOptions {
    pub thumbnail: Thumbnail,
}

/// Writes one document into a ZIP archive.
pub struct DocumentWriter<'o> {
    options: &'o SaveOptions,
}

impl<'o> DocumentWriter<'o> {
    pub fn new(options: &'o SaveOptions) -> DocumentWriter<'o> {
        DocumentWriter { options }
    }

    /// Validates `doc`, then writes it. Nothing is written if validation fails.
    pub fn write(&self, doc: &Document, out: impl Write) -> Result<(), FormatError> {
        doc.validate()?;
        doc.mesh.validate().map_err(FormatError::InvalidMesh)?;
        for f in &doc.foreign {
            paths::check_foreign_path(&f.path)?;
        }
        let extras = Extras::of_document(doc)?;
        let plan = ImagePlan::new(doc);
        let stroke_plan = StrokePlan::new(doc, &plan);
        let stroke_entries = stroke_plan
            .entries()
            .map(|(path, data)| strokes::entry_bytes(data).map(|bytes| (path, bytes)).map_err(|m| FormatError::Unsupported(format!("{path}: {m}"))))
            .collect::<Result<Vec<_>, FormatError>>()?;
        let glb = doc.mesh.to_glb()?;
        let thumbnail = self.options.thumbnail.render(doc);
        let ctx = WriteCtx { plan: &plan, strokes: &stroke_plan, extras: &extras };
        let manifest = manifest::write(doc, &ctx, thumbnail.is_some());

        let mut zip = ZipWriter::new(out);
        zip.add(paths::DOCUMENT, &json_bytes(&manifest), Method::Deflate)?;
        if let Some(r) = &doc.import_report {
            zip.add(paths::IMPORT_REPORT, &json_bytes(&report::write(r)), Method::Deflate)?;
        }
        zip.add(paths::MESH, &glb, Method::Deflate)?;
        drop(glb);
        write_images(&plan, &mut zip)?;
        for (path, bytes) in stroke_entries {
            zip.add(path, &bytes, Method::Deflate)?;
        }
        zip.set_level(Level::Fast);
        for f in &doc.foreign {
            zip.add(&f.path, &f.bytes, Method::Deflate)?;
        }
        if let Some(png) = thumbnail {
            zip.add(paths::THUMBNAIL, &png, Method::Stored)?;
        }
        zip.finish()?.flush()?;
        Ok(())
    }
}

/// Pretty-printed JSON with a final newline.
fn json_bytes(v: &crate::json::Value) -> Vec<u8> {
    let mut s = v.to_string_pretty();
    s.push('\n');
    s.into_bytes()
}

/// Encodes images a batch at a time in parallel and stores them in plan order.
fn write_images<W: Write>(plan: &ImagePlan, zip: &mut ZipWriter<W>) -> Result<(), FormatError> {
    let all: Vec<_> = plan.images().collect();
    for batch in all.chunks(batch_size()) {
        let encoded = par_map(batch, |(_, img)| images::encode(img));
        for ((path, _), bytes) in batch.iter().zip(encoded) {
            let bytes = bytes.map_err(|m| FormatError::image(*path, m))?;
            zip.add(path, &bytes, Method::Stored)?;
        }
    }
    Ok(())
}

/// Runs `write` on a temporary file next to `path`, then renames it over `path`. The temporary
/// file is removed if anything fails.
pub(super) fn save_atomically(path: &Path, write: impl FnOnce(&mut dyn Write) -> Result<(), FormatError>) -> Result<(), FormatError> {
    let name = path.file_name().ok_or_else(|| FormatError::Unsupported(format!("{} is not a file path", path.display())))?;
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(".{}.{}-{stamp}.tmp", name.to_string_lossy(), std::process::id()));
    let result = (|| {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        write(&mut out)?;
        let file = out.into_inner().map_err(|e| e.into_error())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}
