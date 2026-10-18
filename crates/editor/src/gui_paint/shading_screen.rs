//! The shader settings panel's requests on the Paint screen (step 41): edits and undo on the shader settings' own
//! history (a slider drag as one step), the viewport following each change, and the shader files of the asset
//! library's `shaders` folders.

use super::PaintMode;
use super::panels::shader_settings::ShaderEvent;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::history::CoalesceKey;
use materialcraft::shading::ShaderLibrary;
use std::path::PathBuf;

/// Folder of shader files inside an asset library root.
const SHADER_FOLDER: &str = "shaders";

/// The panel's running gesture and the library roots the shader files were loaded from.
#[derive(Default)]
pub struct ShaderPanelState {
    key: Option<CoalesceKey>,
    next_key: u64,
    roots: Option<Vec<PathBuf>>,
}

impl PaintMode {
    pub(super) fn shader_event(&mut self, ev: ShaderEvent) {
        let Some(doc) = &mut self.document else { return };
        let state = &mut self.shader_panel;
        let result = match ev {
            ShaderEvent::Edit { label, edit, gesture } => {
                let key = match (gesture, state.key) {
                    (false, _) => None,
                    (true, Some(k)) => Some(k),
                    (true, None) => {
                        state.next_key = state.next_key.wrapping_add(1);
                        state.key = Some(CoalesceKey(state.next_key));
                        state.key
                    }
                };
                doc.edit_shading(label, key, *edit).map(|_| true)
            }
            ShaderEvent::EndGesture => {
                if state.key.take().is_some() {
                    doc.close_shading_step();
                }
                Ok(false)
            }
            ShaderEvent::Undo => {
                state.key = None;
                doc.undo_shading()
            }
            ShaderEvent::Redo => {
                state.key = None;
                doc.redo_shading()
            }
        };
        match result {
            Ok(true) => self.scene.shading_changed(doc, self.display, self.viewport.gpu()),
            Ok(false) => {}
            Err(e) => self.diag.warn(Area::Project, format!("That shader change was not made: {e}.")),
        }
    }

    /// Loads the shader files of the library roots' `shaders` folders when the roots changed (once at start).
    pub(super) fn sync_shader_library(&mut self) {
        let roots = self.tray.material_folders();
        if self.shader_panel.roots.as_ref() == Some(&roots) {
            return;
        }
        let mut library = ShaderLibrary::default();
        for root in &roots {
            for e in library.load_folder(&root.join(SHADER_FOLDER)) {
                self.diag.warn(Area::General, format!("A shader file can't be loaded: {e}."));
            }
        }
        self.shader_panel.roots = Some(roots);
        self.scene.set_shader_library(library);
        if let Some(doc) = &mut self.document {
            self.scene.shading_changed(doc, self.display, self.viewport.gpu());
        }
    }
}
