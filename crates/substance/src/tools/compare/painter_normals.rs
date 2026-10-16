//! [`PainterNormals`]: the settings that decide how Painter built a project's normal maps, read from the
//! `document.bin` an import keeps under `foreign/substance/`:
//! - the header's format version picks the height → normal filter ([`HeightFilter::for_painter_format`]);
//! - `DataDocument.normalMapSyscoord` is the project's normal map format, in which the baked normal maps are
//!   stored: 0 = OpenGL, 1 = DirectX (Verified on exports: Grappling, AS = 0; Zubzub, Sload Final = 1).

use crate::decode::document::{walk, DocumentHeader, Scalar, ValueTag, Visitor};
use crate::translate::FOREIGN_ROOT;
use materialcraft::paint::height_normal::HeightFilter;
use materialcraft::paint::normal_combine::NormalFormat;
use materialcraft::paint::{Document, OutputOptions};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PainterNormals {
    pub format_version: u32,
    pub baked_format: NormalFormat,
}

impl PainterNormals {
    /// Reads the kept `document.bin`; `None` if the document has none or it cannot be read.
    pub fn of(doc: &Document) -> Option<PainterNormals> {
        let file = doc.foreign_file(&format!("{FOREIGN_ROOT}/paint/document.bin"))?;
        PainterNormals::parse(&file.bytes)
    }

    pub fn parse(data: &[u8]) -> Option<PainterNormals> {
        let format_version = DocumentHeader::parse(data).ok()?.format_version;
        let mut finder = SyscoordFinder::default();
        walk(data, &mut finder).ok()?;
        Some(PainterNormals {
            format_version,
            baked_format: match finder.value {
                Some(1) => NormalFormat::DirectX,
                _ => NormalFormat::OpenGl,
            },
        })
    }

    /// `options` with the filter and baked-map format of this project (strength unchanged).
    pub fn apply(&self, options: &OutputOptions) -> OutputOptions {
        OutputOptions {
            height_filter: HeightFilter::for_painter_format(self.format_version),
            baked_normal_format: self.baked_format,
            ..*options
        }
    }
}

/// Finds `normalMapSyscoord` among the fields of the root object.
#[derive(Default)]
struct SyscoordFinder {
    depth: usize,
    field: Vec<u8>,
    value: Option<u64>,
}

impl Visitor for SyscoordFinder {
    fn object(&mut self, _class: &str) {
        self.depth += 1;
    }

    fn end_object(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn field(&mut self, name: &[u8]) {
        self.field.clear();
        self.field.extend_from_slice(name);
    }

    fn scalar(&mut self, tag: ValueTag, payload: &[u8]) {
        if self.depth == 1 && self.field == b"normalMapSyscoord" {
            self.value = Scalar::decode(tag, payload).and_then(|s| s.as_u64());
        }
    }

    fn unknown_tag(&mut self, _tag: u8, _offset: usize) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_input_gives_none_and_apply_keeps_strength() {
        assert_eq!(PainterNormals::parse(&[1, 2, 3]), None);
        assert_eq!(PainterNormals::of(&Document::default()), None);
        let n = PainterNormals {
            format_version: 59,
            baked_format: NormalFormat::DirectX,
        };
        let base = OutputOptions {
            height_normal_strength: 0.5,
            ..OutputOptions::default()
        };
        let o = n.apply(&base);
        assert_eq!(o.height_filter, HeightFilter::Sobel);
        assert_eq!(o.baked_normal_format, NormalFormat::DirectX);
        assert_eq!(o.height_normal_strength, 0.5);
    }
}
