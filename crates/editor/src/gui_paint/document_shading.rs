//! The open document's shader settings and their own undo history (VS-06): shader edits never appear on the
//! project timeline (ADR 0008 q-undo-stacks), but they make the document unsaved like any other change.

use super::OpenDocument;
use super::SavePoint;
use materialcraft::history::{CoalesceKey, ShaderHistory};
use materialcraft::shading::{ShaderEdit, ShaderEditError};

/// The shader settings' history and where it was saved.
#[derive(Default)]
pub struct ShadingHistory {
    history: ShaderHistory,
    saved: SavePoint,
    saving: Option<SavePoint>,
}

impl ShadingHistory {
    pub fn history(&self) -> &ShaderHistory {
        &self.history
    }

    pub(super) fn is_dirty(&self) -> bool {
        !self.saved.is_current(&self.history)
    }

    pub(super) fn begin_save(&mut self) {
        self.history.close();
        self.saving = Some(SavePoint::of(&self.history));
    }

    pub(super) fn saved(&mut self) {
        if let Some(p) = self.saving.take() {
            self.saved = p;
        }
    }

    pub(super) fn save_failed(&mut self) {
        self.saving = None;
    }
}

impl OpenDocument {
    pub fn shading_history(&self) -> &ShaderHistory {
        self.shading.history()
    }

    /// Applies `edit` to the shader settings as one step of their own history (edits with the same `key` join
    /// the open step: one slider drag). On error nothing changes.
    pub fn edit_shading(&mut self, label: &str, key: Option<CoalesceKey>, edit: ShaderEdit) -> Result<(), ShaderEditError> {
        let h = &mut self.shading;
        let (saved, saving) = (h.saved.is_current(&h.history), h.saving.is_some_and(|p| p.is_current(&h.history)));
        h.history.apply(&mut self.doc.shading, label, key, edit)?;
        h.saved.note_recorded(saved, &h.history);
        if let Some(p) = &mut h.saving {
            p.note_recorded(saving, &h.history);
        }
        Ok(())
    }

    /// Ends the open shader settings step (the end of a slider drag).
    pub fn close_shading_step(&mut self) {
        self.shading.history.close();
    }

    /// Steps the shader settings back once; false if there was nothing to undo.
    pub fn undo_shading(&mut self) -> Result<bool, ShaderEditError> {
        self.shading.history.undo(&mut self.doc.shading)
    }

    /// Steps the shader settings forward once; false if there was nothing to redo.
    pub fn redo_shading(&mut self) -> Result<bool, ShaderEditError> {
        self.shading.history.redo(&mut self.doc.shading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use materialcraft::paint::Document;
    use materialcraft::shading::{ParamValue, ShaderFile, ShaderInstance};
    use std::path::PathBuf;

    #[test]
    fn shader_edits_have_their_own_history_and_make_the_document_unsaved() {
        let mut d = OpenDocument::new(Document::default(), PathBuf::from("a.mcpaint"), None);
        let file = ShaderFile::builtin();
        let inst = ShaderInstance::new("shader", "Shared", &file);
        d.edit_shading("New shader instance", None, ShaderEdit::Insert { at: 0, instance: Box::new(inst), sets: vec!["s".into()] }).unwrap();
        let key = Some(CoalesceKey(3));
        for v in [0.1, 0.2] {
            let edit = ShaderEdit::SetValue { instance: "shader".into(), param: "ao_strength".into(), value: Some(ParamValue::Float(v)) };
            d.edit_shading("Ambient occlusion", key, edit).unwrap();
        }
        d.close_shading_step();
        assert_eq!(d.shading_history().len(), 2);
        assert!(d.history().is_empty(), "nothing on the project timeline");
        assert!(d.is_dirty());
        d.begin_save();
        d.saved_to(PathBuf::from("a.mcpaint"));
        assert!(!d.is_dirty());
        assert!(d.undo_shading().unwrap());
        assert!(d.is_dirty());
        assert_eq!(d.doc().shading.instance("shader").unwrap().stored("ao_strength"), None);
        assert!(d.redo_shading().unwrap());
        assert!(!d.is_dirty(), "back at the saved state");
        assert!(d.edit_shading("x", None, ShaderEdit::Remove { instance: "nope".into(), restore: vec![] }).is_err());
        assert_eq!(d.shading_history().len(), 2);
    }
}
