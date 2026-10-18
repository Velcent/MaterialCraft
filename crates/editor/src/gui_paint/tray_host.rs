//! Hosting the asset tray on the Paint screen: while an asset is dragged the layer panel previews where it
//! would land (LS-19); when it is let go over the stack the drop becomes one layer-stack edit through the
//! history (LS-18), followed by a bake of the maps a smart material or mask needs that the set lacks (AS-05); the tray's library settings go to the settings file (AS-16…AS-18).

use super::PaintMode;
use super::panels::layers::{LayerEvent, StackDropTarget};
use crate::gui_assets::TrayEvent;
use eframe::egui::Pos2;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::Document;
use materialcraft::paint::edit::ops::DropPayload;

impl PaintMode {
    /// Before the panels are drawn: the layer panel learns what is being dragged; the scene learns the library
    /// roots material references are looked up in.
    pub(super) fn before_tray_frame(&mut self) {
        self.layers.hover(self.tray.dragged_payload().cloned());
        let folders = self.tray.material_folders();
        // alpha-image tips are found where materials are
        self.painting.alphas.set_folders(folders.clone());
        self.scene.set_library_folders(folders);
        self.sync_shader_library();
    }

    /// Applies a tray request.
    pub(super) fn apply_tray(&mut self, ev: TrayEvent) {
        match ev {
            TrayEvent::Dropped { item, payload, pos, replace } => {
                self.layers.hover(None);
                if self.drop_on_view(&item, Some(&payload), pos) {
                    return;
                }
                let Some(doc) = &self.document else { return };
                if let Some(ev) = drop_event(&self.layers, doc.doc(), pos, payload, replace) {
                    let set = match &ev {
                        LayerEvent::Drop { target, .. } => Some(target.set().to_string()),
                        _ => None,
                    };
                    self.edit_layers(ev);
                    // A smart material or mask that needs maps the set has not baked gets them baked (AS-05).
                    if let Some(set) = set {
                        self.bake_missing_maps(&set, &item.name, &item.needs);
                    }
                }
            }
            TrayEvent::DroppedImage { item, pos } => {
                self.layers.hover(None);
                if !self.drop_on_view(&item, None, pos) {
                    self.diag.info(Area::Assets, format!("Drop {} on the 3D view to use it as the stencil or projection image.", item.name));
                }
            }
            TrayEvent::DroppedEnvironment { item, pos } => {
                self.layers.hover(None);
                if self.viewport.rect().is_some_and(|r| r.contains(pos)) {
                    self.viewport.set_environment_map(Some(item.path.clone()));
                } else {
                    self.diag.info(Area::Assets, format!("Drop {} on the 3D view to light the view with it.", item.name));
                }
            }
            // normally applied at the end of the tray's frame, where the brush zones are known
            TrayEvent::DroppedBrush { item, brush, pos, image } => self.drop_brush(None, &item, brush, pos, image),
            TrayEvent::SetRoots(roots) => {
                self.shell.settings.update(|s| s.library.roots = roots);
            }
            TrayEvent::SetSavedSearches(saved) => {
                self.shell.settings.update(|s| s.library.saved_searches = saved);
            }
            TrayEvent::SetView(view) => {
                self.shell.settings.update(|s| s.library.tray = view);
            }
        }
    }
}

/// The stack edit for `payload` let go at `pos`, if that is over the stack where the drop does something.
fn drop_event(stack: &dyn StackDropTarget, doc: &Document, pos: Pos2, payload: DropPayload, replace: bool) -> Option<LayerEvent> {
    let target = stack.target_at(doc, pos, &payload)?;
    Some(LayerEvent::Drop { target, payload, replace })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui_paint::document_state::OpenDocument;
    use crate::gui_paint::stack_editor;
    use materialcraft::material_fill::{FolderResolver, MaterialCache, MaterialFills};
    use materialcraft::paint::edit::LayerSlot;
    use materialcraft::paint::edit::ops::{DropTarget, MaterialAsset};
    use materialcraft::paint::{BlendMode, Channel, FillContext, Layer, LayerKind, MaterialMaps, OutputOptions, PaintContent, TextureSet};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A stack that accepts every drop at one fixed target (the layer panel's own hit testing is tested there).
    struct Fixed(DropTarget);

    impl StackDropTarget for Fixed {
        fn target_at(&self, doc: &Document, _: Pos2, payload: &DropPayload) -> Option<DropTarget> {
            materialcraft::paint::edit::ops::accepts(doc, &self.0, payload).then(|| self.0.clone())
        }

        fn hover(&mut self, _: Option<DropPayload>) {}
    }

    fn open() -> OpenDocument {
        let mut set = TextureSet::new("s", "S", (64, 64)).with_channel(Channel::BaseColor).with_channel(Channel::Roughness);
        set.layers.push(Layer::new("base", "Base", LayerKind::Paint(PaintContent::default())).with_blend(Channel::BaseColor, BlendMode::Normal, 1.0));
        let mut doc = Document::default();
        doc.texture_sets.push(set);
        OpenDocument::new(doc, PathBuf::from("test.mcpaint"), None)
    }

    /// The gate: a material dragged from the tray and let go between layers is one history step that adds a fill
    /// layer with the material, which composites from the library folder; undo takes it back exactly.
    #[test]
    fn a_dropped_material_is_one_undo_step_and_composites() {
        let lib = std::env::temp_dir().join(format!("mc-tray-host-{}", std::process::id()));
        std::fs::create_dir_all(lib.join("materials")).unwrap();
        std::fs::write(lib.join("materials").join("brick.mca"), materialcraft::graph::EXAMPLE).unwrap();

        let mut d = open();
        let before = d.doc().clone();
        let payload = DropPayload::Material(MaterialAsset { name: "Brick".into(), path: "materials/brick.mca".into(), params: Vec::new() });
        let stack = Fixed(DropTarget::Between { set: "s".into(), at: LayerSlot::top(1) });
        let ev = drop_event(&stack, d.doc(), Pos2::ZERO, payload.clone(), false).expect("accepted");
        let applied = stack_editor::apply(&mut d, ev).unwrap();
        assert_eq!(d.history().len(), 1, "one step");
        assert_eq!(applied.set, Some(0));
        let layer = applied.select.and_then(|ids| ids.first().cloned()).expect("the new layer is selected");
        assert_eq!(d.doc().texture_sets[0].layer(&layer).map(|l| l.name.as_str()), Some("Brick"));

        let fills = MaterialFills::new(Arc::new(FolderResolver::new([PathBuf::from("/nonexistent"), lib.clone()])), Arc::new(MaterialCache::default()));
        let set = &d.doc().texture_sets[0];
        let maps = MaterialMaps::build_with_fills(set, &OutputOptions::for_document(d.doc()), &FillContext::with_sources(&fills));
        assert!(maps.warnings.is_empty(), "the material was found and evaluated: {:?}", maps.warnings);
        let plain = MaterialMaps::build(&before.texture_sets[0], &OutputOptions::for_document(&before));
        assert_ne!(maps.base_color.px, plain.base_color.px, "the material shows in the base colour");

        d.undo().unwrap();
        assert_eq!(d.doc(), &before, "undo restores the document");
        assert!(
            drop_event(&Fixed(DropTarget::OntoMask(materialcraft::paint::edit::LayerRef::new("s", "base"))), d.doc(), Pos2::ZERO, payload, false).is_none()
        );
        let _ = std::fs::remove_dir_all(&lib);
    }
}
