//! The viewport eyedropper's samples (CP-06): the colour picker turns a click on the view into a request; the
//! displayed colour is read back from the GPU's next frame, the stored value (Shift) goes through the view projector
//! and the shared texel lookup ([`TexelProbe`]) on the composite the view shows. In the 2D view the displayed colour
//! is read back from its own frame and the stored value is looked up at the UV under the pointer.

use super::PaintMode;
use crate::gui_color::{Sample, Source};
use eframe::egui;
use materialcraft::diag::{Area, SinkExt};
use materialcraft::paint::Channel;
use materialcraft::paint::FillContext;
use materialcraft::paint::probe::TexelProbe;
use materialcraft::projection::ViewProjector;

use super::view2d::in_working_area;

/// Frames to wait for the GPU's colour read-back before giving up.
const READ_FRAMES: u64 = 30;

impl PaintMode {
    /// Answers the eyedropper: a pending GPU read first, else this frame's click on the view. Call once per frame
    /// after the picker's frame.
    pub(super) fn eyedrop(&mut self, ctx: &egui::Context) {
        if let Some((asked, uv)) = self.color_read {
            let (shot, frames) = match self.viewport.gpu().lock() {
                Ok(mut g) if uv => (g.uv.color.take(), g.uv.frames),
                Ok(mut g) => (g.color.take(), g.frames),
                Err(_) => (None, u64::MAX),
            };
            match shot {
                Some(shot) => {
                    self.color_read = None;
                    match shot.rgba {
                        Some(px) => self.color.accept_sample(ctx, Sample::displayed(px)),
                        None => self.sample_failed("The colour shown there could not be read from the view."),
                    }
                }
                None if frames > asked.saturating_add(READ_FRAMES) => {
                    self.color_read = None;
                    self.sample_failed("The view did not answer the eyedropper.");
                }
                None => ctx.request_repaint(),
            }
            return;
        }
        let (mesh, uv) = (self.viewport.rect(), self.viewport.uv_rect());
        let area = match (mesh, uv) {
            (Some(a), Some(b)) => a.union(b),
            (a, b) => match a.or(b) {
                Some(r) => r,
                None => return,
            },
        };
        let Some(req) = self.color.poll_eyedropper(ctx, area) else { return };
        let (view, in_uv) = match uv.filter(|r| r.contains(req.at)) {
            Some(r) => (r, true),
            None => match mesh {
                Some(r) => (r, false),
                None => return,
            },
        };
        let local = [req.at.x - view.left(), req.at.y - view.top()];
        match req.source {
            Source::Stored => {
                let sample = if in_uv { self.sample_stored_uv(local, req.channel) } else { self.sample_stored(local, req.channel) };
                match sample {
                    Ok(sample) => self.color.accept_sample(ctx, sample),
                    Err(why) => self.sample_failed(&why),
                }
            }
            Source::Displayed => {
                if let Ok(mut g) = self.viewport.gpu().lock() {
                    if in_uv {
                        (g.uv.color, g.uv.want_color) = (None, Some(local));
                        self.color_read = Some((g.uv.frames, true));
                    } else {
                        (g.color, g.want_color) = (None, Some(local));
                        self.color_read = Some((g.frames, false));
                    }
                }
                ctx.request_repaint();
            }
        }
    }

    /// `channel`'s stored value at the texel under `local` (2D view coordinates) of the set the 2D view shows.
    fn sample_stored_uv(&self, local: [f32; 2], channel: Channel) -> Result<Sample, String> {
        let doc = self.document.as_ref().ok_or("Open a document to pick from.")?;
        let map = self.viewport.uv_map().ok_or("The 2D view is not shown.")?;
        let uv = map.uv(local);
        if !in_working_area(uv) {
            return Err("The eyedropper picks inside the 0–1 square of the 2D view.".into());
        }
        let set =
            self.layers.current_set(doc.doc()).and_then(|s| doc.doc().texture_sets.iter().position(|t| t.id == s.id)).ok_or("No texture set is shown.")?;
        self.with_set_fills(set, |probe| {
            let at = probe.texel_of(set, uv).ok_or("There is no texel under the eyedropper.")?;
            let value = probe.stored(at, channel).ok_or_else(|| format!("This texture set has no {} channel.", channel.id()))?;
            let [r, g, b, _] = value.value;
            Ok(Sample { rgb: [r, g, b], space: value.color_space })
        })
    }

    /// `channel`'s stored value at the texel under `local` (view coordinates), composited as the view's maps are.
    fn sample_stored(&self, local: [f32; 2], channel: Channel) -> Result<Sample, String> {
        self.with_stored_probe(local, "the eyedropper", |probe, view| {
            let hit = view.pick(local).ok_or("There is no surface under the eyedropper.")?;
            let at = probe.locate(&hit).ok_or("No texture set paints the surface under the eyedropper.")?;
            let value = probe.stored(at, channel).ok_or_else(|| format!("This texture set has no {} channel.", channel.id()))?;
            let [r, g, b, _] = value.value;
            Ok(Sample { rgb: [r, g, b], space: value.color_space })
        })
    }

    /// The stored-value lookup the eyedropper (CP-06) and the material picker (PT-18) share (ADR 0008
    /// mc2-q-pt-08c): `f` gets a [`TexelProbe`] on the composite the view shows (with the fills of the texture set
    /// under `local`, view coordinates) and the view's projector. `who` names the tool in messages.
    pub(super) fn with_stored_probe<R>(
        &self,
        local: [f32; 2],
        who: &str,
        f: impl FnOnce(&TexelProbe, &dyn ViewProjector) -> Result<R, String>,
    ) -> Result<R, String> {
        let doc = self.document.as_ref().ok_or("Open a document to pick from.")?;
        let view = self.view_projector().ok_or("The view is still being prepared; try again in a moment.")?;
        let hit = view.pick(local).ok_or_else(|| format!("There is no surface under {who}."))?;
        let at = TexelProbe::new(doc.doc()).locate(&hit).ok_or_else(|| format!("No texture set paints the surface under {who}."))?;
        self.with_set_fills(at.set, |probe| f(probe, &view))
    }

    /// `f` with a [`TexelProbe`] on the composite the views show, with texture set `set`'s fills: its surface map as
    /// the maps worker built it (fills that need one are left out until it is).
    fn with_set_fills<R>(&self, set: usize, f: impl FnOnce(&TexelProbe) -> Result<R, String>) -> Result<R, String> {
        let doc = self.document.as_ref().ok_or("Open a document to pick from.")?;
        let materials = self.scene.material_fills(doc);
        let needs = doc.doc().texture_sets.get(set).filter(|s| s.needs_surface());
        let surface = needs.and_then(|s| self.scene.fill_surfaces()?.cached(s, s.resolution));
        let fills = surface.as_ref().map_or_else(|| FillContext::with_sources(&materials), |s| s.context(&materials));
        f(&TexelProbe::new(doc.doc()).with_fills(&fills))
    }

    fn sample_failed(&mut self, why: &str) {
        self.color.sample_failed();
        self.diag.info(Area::General, why.to_owned());
    }
}
