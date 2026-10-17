//! The viewport eyedropper's samples (CP-06): the colour picker turns a click on the view into a request; the
//! displayed colour is read back from the GPU's next frame, the stored value (Shift) goes through the view projector
//! and the shared texel lookup ([`TexelProbe`]) on the composite the view shows.

use super::PaintMode;
use crate::gui_color::{Sample, Source};
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::Channel;
use materialcraft::paint::FillContext;
use materialcraft::paint::probe::TexelProbe;
use materialcraft::projection::ViewProjector;

/// Frames to wait for the GPU's colour read-back before giving up.
const READ_FRAMES: u64 = 30;

impl PaintMode {
    /// Answers the eyedropper: a pending GPU read first, else this frame's click on the view. Call once per frame
    /// after the picker's frame.
    pub(super) fn eyedrop(&mut self, ctx: &egui::Context) {
        if let Some(asked) = self.color_read {
            let shot = self.viewport.gpu().lock().ok().and_then(|mut g| g.color.take());
            match shot {
                Some(shot) => {
                    self.color_read = None;
                    match shot.rgba {
                        Some(px) => self.color.accept_sample(ctx, Sample::displayed(px)),
                        None => self.sample_failed("The colour shown there could not be read from the view."),
                    }
                }
                None if self.viewport.frames() > asked + READ_FRAMES => {
                    self.color_read = None;
                    self.sample_failed("The view did not answer the eyedropper.");
                }
                None => ctx.request_repaint(),
            }
            return;
        }
        let Some(view) = self.viewport.rect() else { return };
        let Some(req) = self.color.poll_eyedropper(ctx, view) else { return };
        let local = [req.at.x - view.left(), req.at.y - view.top()];
        match req.source {
            Source::Stored => match self.sample_stored(local, req.channel) {
                Ok(sample) => self.color.accept_sample(ctx, sample),
                Err(why) => self.sample_failed(&why),
            },
            Source::Displayed => {
                if let Ok(mut g) = self.viewport.gpu().lock() {
                    g.color = None;
                    g.want_color = Some(local);
                }
                self.color_read = Some(self.viewport.frames());
                ctx.request_repaint();
            }
        }
    }

    /// `channel`'s stored value at the texel under `local` (view coordinates), composited as the view's maps are.
    fn sample_stored(&self, local: [f32; 2], channel: Channel) -> Result<Sample, String> {
        let doc = self.document.as_ref().ok_or("Open a document to pick from.")?;
        let view = self.view_projector().ok_or("The view is still being prepared; try again in a moment.")?;
        let hit = view.pick(local).ok_or("There is no surface under the eyedropper.")?;
        let materials = self.scene.material_fills(doc);
        let at = TexelProbe::new(doc.doc()).locate(&hit).ok_or("No texture set paints the surface under the eyedropper.")?;
        // the set's surface map as the view's maps worker built it (fills that need one are left out until it is)
        let set = doc.doc().texture_sets.get(at.set).filter(|s| s.needs_surface());
        let surface = set.and_then(|s| self.scene.fill_surfaces()?.cached(s, s.resolution));
        let fills = surface.as_ref().map_or_else(|| FillContext::with_sources(&materials), |s| s.context(&materials));
        let probe = TexelProbe::new(doc.doc()).with_fills(&fills);
        let value = probe.stored(at, channel).ok_or_else(|| format!("This texture set has no {} channel.", channel.id()))?;
        let [r, g, b, _] = value.value;
        Ok(Sample { rgb: [r, g, b], space: value.color_space })
    }

    fn sample_failed(&mut self, why: &str) {
        self.color.sample_failed();
        self.diag.info(Area::General, why.to_owned());
    }
}
