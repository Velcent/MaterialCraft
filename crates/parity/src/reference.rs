//! [`References`]: Painter's result a subject is compared against, read from the project at run time and
//! put into our conventions (rows top first, tangent normals OpenGL). Never written anywhere but the
//! report's cache folder.

use crate::image_ops::flip_green;
use crate::project::{OpenedProject, SetCase};
use crate::subject::{CaseError, ReferenceKind};
use materialcraft::paint::BakedKind;
use materialcraft::tex::Tex;
use materialcraft_substance::translate::{BakedMaps, BakedSource};

/// One loaded reference image.
pub struct Reference {
    pub image: Tex,
    /// Painter's name for it (`Ambient Occlusion Map from Mesh Cup`).
    pub name: String,
    /// Size as stored in the project (the image may have been shrunk since).
    pub stored_size: (usize, usize),
    pub notes: Vec<String>,
}

pub struct References<'a> {
    project: &'a OpenedProject,
    baked: BakedMaps<'a>,
}

impl<'a> References<'a> {
    pub fn new(project: &'a OpenedProject) -> References<'a> {
        References {
            project,
            baked: BakedMaps::new(&project.spp),
        }
    }

    /// The baked maps Painter stored for a set.
    pub fn baked_sources(&self, set: &SetCase) -> Vec<BakedSource> {
        self.baked.sources(&set.name, &set.refs)
    }

    /// True if the project holds the reference for `kind` (cheap: nothing is decoded).
    pub fn has(&self, set: &SetCase, kind: &ReferenceKind) -> bool {
        match kind {
            ReferenceKind::Baked(k) => self.baked_sources(set).iter().any(|s| s.kind == *k),
            ReferenceKind::Cache => false,
        }
    }

    pub fn load(&self, set: &SetCase, kind: &ReferenceKind) -> Result<Reference, CaseError> {
        match kind {
            ReferenceKind::Baked(k) => self.load_baked(set, k),
            ReferenceKind::Cache => Err(CaseError::skipped(
                "texture-cache cases are measured through the layer stack",
            )),
        }
    }

    fn load_baked(&self, set: &SetCase, kind: &BakedKind) -> Result<Reference, CaseError> {
        let source = self
            .baked_sources(set)
            .into_iter()
            .find(|s| s.kind == *kind)
            .ok_or_else(|| CaseError::skipped(format!("Painter stored no {} map for this set", kind.id())))?;
        let read = self
            .baked
            .read(&source)
            .map_err(|e| CaseError::failed(format!("reading {}: {e}", source.name)))?;
        let mut notes = Vec::new();
        if read.missing_bytes > 0 {
            notes.push(format!(
                "reference block short by {} bytes (padded)",
                read.missing_bytes
            ));
        }
        let mut image = read.map.image.to_tex();
        if *kind == BakedKind::Normal && self.project.directx_normals {
            flip_green(&mut image);
            notes.push("reference stored DirectX; green flipped to OpenGL".into());
        }
        Ok(Reference {
            stored_size: (image.w, image.h),
            image,
            name: source.name,
            notes,
        })
    }
}
