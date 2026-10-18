//! Shader settings in their own history: [`ShaderEdit`] as a [`Command`] on [`ShaderSettings`] (VS-06). It holds
//! no shared allocations, so the byte budget never drops its steps.

use super::Command;
use crate::shading::{ShaderEdit, ShaderEditError, ShaderSettings};

impl Command for ShaderEdit {
    type Target = ShaderSettings;
    type Error = ShaderEditError;

    fn apply(self, settings: &mut ShaderSettings) -> Result<ShaderEdit, ShaderEditError> {
        ShaderEdit::apply(self, settings)
    }

    fn absorb(&mut self, later: ShaderEdit) -> Option<ShaderEdit> {
        ShaderEdit::absorb(self, later)
    }
}
