//! [`DocumentStroke`]: a stroke into a paint document, with everything the history needs to make it one undo step
//! (PT-54): the edit that prepared the target (a mask's new paint effect), the old tiles of every image it painted
//! and the stroke appended to the target's stroke record.
//!
//! The document is lent in pieces so the caller can hold a view of the mesh while the texture sets are painted:
//! [`DocumentStroke::prepare`] (whole document) → [`Prepared::start`], [`DocumentStroke::add`],
//! [`DocumentStroke::finish`] (the texture set) → [`Ended::record`] (whole document, returns the inverses).

use crate::BrushError;
use crate::engine::InputPoint;
use crate::session::{Finished, SessionScene, StrokeSession, StrokeSetup};
use crate::target::{PaintTarget, Resolved};
use materialcraft::mesh::Mesh;
use materialcraft::paint::edit::{Edit, EditError, TileCapture};
use materialcraft::paint::stroke::{StrokeOwner, StrokeTool};
use materialcraft::paint::{Document, TextureSet};

/// A target made ready for painting: preparation applied, images resolved, old tiles held.
pub struct Prepared {
    target: PaintTarget,
    resolved: Resolved,
    inverses: Vec<Edit>,
    captures: Vec<TileCapture>,
}

/// A stroke in progress on a document.
pub struct DocumentStroke {
    target: PaintTarget,
    resolved: Resolved,
    erase: bool,
    inverses: Vec<Edit>,
    captures: Vec<TileCapture>,
    session: StrokeSession,
}

/// A stroke whose painting is done; [`Ended::record`] completes it.
pub struct Ended {
    target: PaintTarget,
    owner: StrokeOwner,
    inverses: Vec<Edit>,
    captures: Vec<TileCapture>,
    finished: Option<Finished>,
}

impl DocumentStroke {
    /// Makes `target` ready in `doc` for painting `setup.values`.
    pub fn prepare(doc: &mut Document, target: PaintTarget, values: &crate::target::PaintValues) -> Result<Prepared, BrushError> {
        let set = doc.texture_set(&target.layer.set).ok_or(BrushError::NotPaintable("the texture set is gone"))?;
        let mut inverses = Vec::new();
        if let Some(edit) = target.preparation(set)? {
            inverses.push(edit.apply(doc).map_err(BrushError::Edit)?);
        }
        let set = doc.texture_set(&target.layer.set).ok_or(BrushError::NotPaintable("the texture set is gone"))?;
        let resolved = target.resolve(set, values)?;
        let captures =
            resolved.images.iter().map(|i| TileCapture::begin(doc, i.image.clone())).collect::<Result<Vec<_>, EditError>>().map_err(BrushError::Edit)?;
        Ok(Prepared { target, resolved, inverses, captures })
    }

    /// Paints `points` (see [`StrokeSession::add`]); returns the changed tiles of the set's images.
    pub fn add(&mut self, scene: &SessionScene, points: &[InputPoint], set: &mut TextureSet) -> Result<Vec<(u32, u32)>, BrushError> {
        let mut targets = self.target.channel_targets(set, &self.resolved, self.erase)?;
        self.session.add(scene, points, &mut targets)
    }

    /// Paints the stroke's last dabs. The result's [`Ended::record`] must follow, also after an error here (the
    /// pixels painted so far stay and are recorded).
    pub fn finish(self, scene: &SessionScene, set: &mut TextureSet) -> (Ended, Result<Vec<(u32, u32)>, BrushError>) {
        let DocumentStroke { target, resolved, erase, inverses, captures, session } = self;
        let result = target.channel_targets(set, &resolved, erase).and_then(|mut t| session.finish(scene, &mut t));
        let (finished, out) = match result {
            Ok(f) => {
                let touched = f.touched.clone();
                (Some(f), Ok(touched))
            }
            Err(e) => (None, Err(e)),
        };
        (Ended { owner: resolved.owner, target, inverses, captures, finished }, out)
    }

    /// The stroke's camera (3D-view strokes).
    pub fn camera(&self) -> Option<materialcraft::camera::Camera> {
        self.session.camera()
    }
}

impl Prepared {
    /// Starts painting with `setup` through `scene`; `mesh` and `shown` draw the occlusion depth.
    pub fn start(
        self,
        setup: StrokeSetup,
        scene: &SessionScene,
        mesh: &Mesh,
        shown: &dyn Fn(usize) -> bool,
        set: &mut TextureSet,
    ) -> Result<DocumentStroke, Box<(Ended, BrushError)>> {
        let erase = setup.tool == StrokeTool::Eraser;
        let Prepared { target, resolved, inverses, captures } = self;
        let session = target.channel_targets(set, &resolved, erase).and_then(|t| StrokeSession::begin(setup, scene, mesh, shown, &t));
        match session {
            Ok(session) => Ok(DocumentStroke { target, resolved, erase, inverses, captures, session }),
            Err(e) => Err(Box::new((Ended { owner: resolved.owner, target, inverses, captures, finished: None }, e))),
        }
    }

    /// What the stroke resolved to (images painted, channels the layer lacks).
    pub fn resolved(&self) -> &Resolved {
        &self.resolved
    }
}

impl Ended {
    /// Appends the stroke to the target's stroke record and returns every inverse in the order the edits were made,
    /// for one history step. A record that can't take the stroke (unreadable) is left as it is; the error comes
    /// back with the inverses so the pixels are still undoable.
    pub fn record(self, doc: &mut Document) -> (Vec<Edit>, Option<EditError>) {
        let Ended { target, owner, mut inverses, captures, finished } = self;
        let mut problem = None;
        for c in captures {
            match c.finish(doc) {
                Ok(Edit::SwapTiles { tiles, .. }) if tiles.is_empty() => {}
                Ok(inv) => inverses.push(inv),
                Err(e) => problem = Some(e),
            }
        }
        if let Some(f) = finished.filter(|f| f.dabs > 0) {
            match (Edit::AppendStroke { layer: target.layer.clone(), owner, stroke: Box::new(f.stroke) }).apply(doc) {
                Ok(inv) => inverses.push(inv),
                Err(e) => problem = Some(e),
            }
        }
        (inverses, problem)
    }
}
