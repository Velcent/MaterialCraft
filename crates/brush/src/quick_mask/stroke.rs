//! [`QuickMaskStroke`]: a brush stroke into a texture set's quick mask (the edit state of PT-42): the paint tool
//! protects, the eraser frees. One stroke is one step of the quick mask's own undo history.

use super::QuickMask;
use crate::BrushError;
use crate::engine::InputPoint;
use crate::session::{Finished, SessionScene, StrokeSession, StrokeSetup};
use materialcraft::mesh::Mesh;
use materialcraft::paint::stroke::StrokeTool;
use materialcraft::stamp::{ChannelTarget, StampMode};

/// A stroke in progress into one texture set's quick mask.
pub struct QuickMaskStroke {
    session: StrokeSession,
    set: String,
    mode: StampMode,
}

fn target(image: &mut materialcraft::paint::TiledImage, mode: StampMode) -> [ChannelTarget<'_>; 1] {
    [ChannelTarget { image, value: [1.0; 4], mode, srgb: false }]
}

impl QuickMaskStroke {
    /// Starts a stroke into the mask of texture set `set` (resolution `size`), keeping the mask as it was for undo.
    pub fn begin(
        mask: &mut QuickMask,
        set: &str,
        size: (u32, u32),
        setup: StrokeSetup,
        scene: &SessionScene,
        mesh: &Mesh,
        shown: &dyn Fn(usize) -> bool,
    ) -> Result<QuickMaskStroke, BrushError> {
        let mode = if setup.tool == StrokeTool::Eraser { StampMode::Erase } else { StampMode::Paint };
        let image = mask.begin_edit(set, size).ok_or(BrushError::NotPaintable("the quick mask can't have the texture set's size"))?;
        match StrokeSession::begin(setup, scene, mesh, shown, &target(image, mode)) {
            Ok(session) => Ok(QuickMaskStroke { session, set: set.to_string(), mode }),
            Err(e) => {
                // nothing was painted: the undo step just made goes again
                mask.undo();
                mask.redo.clear();
                Err(e)
            }
        }
    }

    /// The texture set whose mask is painted.
    pub fn set(&self) -> &str {
        &self.set
    }

    /// Paints `points`; returns the changed tiles.
    pub fn add(&mut self, mask: &mut QuickMask, scene: &SessionScene, points: &[InputPoint]) -> Result<Vec<(u32, u32)>, BrushError> {
        let image = mask.image_mut(&self.set).ok_or(BrushError::NotPaintable("the quick mask was cleared"))?;
        self.session.add(scene, points, &mut target(image, self.mode))
    }

    /// Paints the last dabs.
    pub fn finish(self, mask: &mut QuickMask, scene: &SessionScene) -> Result<Finished, BrushError> {
        let image = mask.image_mut(&self.set).ok_or(BrushError::NotPaintable("the quick mask was cleared"))?;
        self.session.finish(scene, &mut target(image, self.mode))
    }

    /// The stroke's camera (3D-view strokes).
    pub fn camera(&self) -> Option<materialcraft::camera::Camera> {
        self.session.camera()
    }
}
