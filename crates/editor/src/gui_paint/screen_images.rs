//! The Paint screen's side of the images over the view and of the material picker (PT-15, PJ-26, PT-18): the
//! stencil key's gizmo drags and the hide key, loading an image from a file or from the asset tray (a picture, or a
//! material graph for the projection tool), the brush panel's image requests, and the picker's click, which reads
//! every channel through the eyedropper's lookup and hands back to the previous tool. The state lives in the brush
//! tools ([`super::tools::screen_image`]).

use super::PaintMode;
use super::input::StrokeInput;
use super::shell::actions;
use super::shell::tools::Tool;
use super::tools::material_picker;
use super::tools::screen_image::{ImageSlot, ScreenImage};
use crate::gui_assets::results::Item;
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::edit::ops::DropPayload;
use materialcraft::projection::Repeat;
use std::path::Path;

/// Side of a material graph rendered for projection painting.
const MATERIAL_SIZE: usize = 1024;

/// Largest picture file read.
const MAX_IMAGE_BYTES: u64 = 512 << 20;

/// What the brush panel asks of an image over the view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageEdit {
    /// Choose a PNG file.
    Load,
    Clear,
    /// Back to fitted on the view, unturned.
    Reset,
    Repeat(Repeat),
}

impl PaintMode {
    /// The slot the gizmo and drops act on with the current tool.
    fn image_slot(&self) -> ImageSlot {
        if self.shell.tools.current == Tool::Projection { ImageSlot::Projection } else { ImageSlot::Stencil }
    }

    /// This frame's stencil keys: the gizmo drag on the current slot's image, the hide key. Call after the viewport
    /// was drawn.
    pub(super) fn screen_image_input(&mut self, ctx: &egui::Context) {
        self.painting.images.hidden = ctx.input(|i| self.shell.shortcuts.held(i, actions::STENCIL_HIDE));
        let slot = self.image_slot();
        match (self.viewport.image_drag(), self.viewport.rect()) {
            (Some(d), Some(r)) => {
                if self.painting.images.drag(slot, d.action, d.at, d.started, d.snap, [r.width(), r.height()]) {
                    ctx.request_repaint();
                }
            }
            _ => self.painting.images.end_drag(),
        }
    }

    /// Draws the images over the view.
    pub(super) fn draw_screen_images(&mut self, ctx: &egui::Context) {
        let (Some(painter), Some(rect)) = (self.viewport.overlay_painter(ctx), self.viewport.rect()) else { return };
        let projection = self.shell.tools.current == Tool::Projection;
        self.painting.images.draw(ctx, &painter, rect, projection, self.viewport.image_key());
    }

    /// A brush panel request for `slot`'s image.
    pub(super) fn screen_image_event(&mut self, slot: ImageSlot, edit: ImageEdit) {
        match edit {
            ImageEdit::Load => {
                let Some(path) = rfd::FileDialog::new().add_filter("PNG image", &["png"]).pick_file() else { return };
                self.load_picture(slot, &path);
            }
            ImageEdit::Clear => self.painting.images.set(slot, None),
            ImageEdit::Reset => {
                if let Some(image) = self.painting.images.get_mut(slot) {
                    image.reset();
                }
            }
            ImageEdit::Repeat(r) => {
                if let Some(image) = self.painting.images.get_mut(slot) {
                    image.repeat = r;
                }
            }
        }
    }

    fn load_picture(&mut self, slot: ImageSlot, path: &Path) {
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        match read_png(path).and_then(|tex| ScreenImage::picture(name.clone(), tex)) {
            Ok(image) => {
                self.painting.images.set(slot, Some(image));
                self.diag.info(Area::General, format!("{name} is the {} now.", slot.label()));
            }
            Err(e) => self.diag.warn(Area::General, format!("{} can't be the {}: {e}.", path.display(), slot.label())),
        }
    }

    /// An asset let go at `pos`: over the view, a picture becomes the stencil (the projection image while the
    /// projection tool is on), a material graph the projection image. Returns whether it was taken.
    pub(super) fn drop_on_view(&mut self, item: &Item, payload: Option<&DropPayload>, pos: egui::Pos2) -> bool {
        if !self.viewport.rect().is_some_and(|r| r.contains(pos)) {
            return false;
        }
        match payload {
            None => {
                let Some(dir) = &item.root_dir else { return false };
                self.load_picture(self.image_slot(), &dir.join(&item.path));
                true
            }
            Some(DropPayload::Material(m)) => {
                let Some(doc) = &self.document else { return false };
                let fills = self.scene.material_fills(doc);
                match fills.render(&m.path, &m.params, MATERIAL_SIZE) {
                    Ok(rendered) => {
                        self.painting.images.set(ImageSlot::Projection, Some(ScreenImage::material(m.name.clone(), rendered)));
                        self.diag.info(Area::General, format!("{} is the projection image now; the projection tool paints it.", m.name));
                    }
                    Err(e) => self.diag.warn(Area::General, format!("{} can't be projected: {e}.", m.name)),
                }
                true
            }
            Some(_) => false,
        }
    }

    /// The material picker (PT-18): a click reads every channel under it into the brush values and hands back to
    /// the tool before. Takes the stroke input while the picker is the tool; returns it untouched otherwise.
    pub(super) fn material_pick(&mut self, input: Vec<StrokeInput>) -> Vec<StrokeInput> {
        if self.shell.tools.current != Tool::MaterialPicker {
            return input;
        }
        let at = input.iter().find_map(|ev| match ev {
            StrokeInput::Down(s) | StrokeInput::Line { to: s, .. } => Some(s.pos),
            _ => None,
        });
        let Some(at) = at else { return Vec::new() };
        match self.with_stored_probe(at, "the material picker", |probe, view| material_picker::pick(probe, view, at).map_err(str::to_owned)) {
            Ok(picked) => {
                self.painting.brush.take_material(&picked);
                let names: Vec<String> = picked.iter().map(|v| v.channel.id()).collect();
                self.diag.info(Area::General, format!("Picked {}.", names.join(", ")));
                self.shell.tools.select(self.shell.tools.previous);
            }
            Err(why) => self.diag.info(Area::General, why),
        }
        Vec::new()
    }
}

fn read_png(path: &Path) -> Result<materialcraft::tex::Tex, String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_IMAGE_BYTES {
        return Err("the file is too large".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    materialcraft::png::decode(&bytes)
}
