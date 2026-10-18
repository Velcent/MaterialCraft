//! Images laid over the 3D view (PT-15, PJ-26): the stencil every brush stroke is masked by, and the image or
//! material the projection tool paints. Both are placed by one [`ScreenImageTransform`] each and edited with the
//! same gizmo ([`gizmo`]); [`draw`] shows them over the view.
//!
//! - [`ScreenImage`]: one image (a PNG from a file or the asset tray, or a material graph rendered once), its
//!   placement (fitted to the view when first used) and its repetition.
//! - [`ScreenImages`]: the stencil and the projection image of the session, whether the stencil is hidden (while
//!   its key is held) and the gizmo drag in progress.
//!
//! Neither has symmetry (ADR 0008 mc2-q-pt-04c): strokes through them are painted without symmetric copies.

pub mod draw;
pub mod gizmo;

use eframe::egui;
use gizmo::{GizmoAction, GizmoDrag};
use materialcraft::material_fill::RenderedMaterial;
use materialcraft::mesh::SurfaceMap;
use materialcraft::paint::{Channel, PixelFormat, TiledImage};
use materialcraft::projection::{Filter, ImageLevels, LevelChain, Repeat, ScreenImageTransform, Texels, ViewProjector};
use materialcraft::stamp::{ProjectedPaint, Stencil};
use materialcraft::tex::Tex;
use std::sync::Arc;

/// Which image a request concerns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageSlot {
    Stencil,
    Projection,
}

impl ImageSlot {
    pub fn label(self) -> &'static str {
        match self {
            ImageSlot::Stencil => "stencil",
            ImageSlot::Projection => "projection image",
        }
    }
}

/// What an image shows.
enum Source {
    /// A picture: its colour (as stored, sRGB-encoded) and its grey (luminance), each with prefiltered levels; the
    /// grey also as a texture-space image for the stencil lookup.
    Picture { colour: Tex, colour_levels: LevelChain, grey: Tex, grey_levels: LevelChain, stencil: TiledImage },
    /// A material graph rendered once: one image per channel it sets.
    Material(Arc<RenderedMaterial>),
}

/// One image over the view.
pub struct ScreenImage {
    pub name: String,
    source: Source,
    /// Where it lies on the view; `None` until first shown (then fitted to the view).
    placement: Option<ScreenImageTransform>,
    pub repeat: Repeat,
    /// What [`draw`] shows, made once.
    preview: Tex,
    texture: Option<egui::TextureHandle>,
}

impl ScreenImage {
    /// A picture (straight RGBA as a PNG decodes, colour sRGB-encoded); `None` for an empty one.
    pub fn picture(name: impl Into<String>, colour: Tex) -> Result<ScreenImage, String> {
        if colour.w == 0 || colour.h == 0 || colour.px.len() != colour.w * colour.h {
            return Err("the image is empty".into());
        }
        let grey = Tex { w: colour.w, h: colour.h, px: colour.px.iter().map(|p| luminance(*p)).collect() };
        let mut stencil = TiledImage::new(grey.w as u32, grey.h as u32, PixelFormat::from_id("gray32f").ok_or("no grey format")?).map_err(|e| e.to_string())?;
        for (i, p) in grey.px.iter().enumerate() {
            stencil.set((i % grey.w) as u32, (i / grey.w) as u32, *p);
        }
        let (colour_levels, grey_levels) = (LevelChain::build(&colour), LevelChain::build(&grey));
        let preview = colour.clone();
        let source = Source::Picture { colour, colour_levels, grey, grey_levels, stencil };
        Ok(ScreenImage { name: name.into(), source, placement: None, repeat: Repeat::None, preview, texture: None })
    }

    /// A rendered material: it paints every channel it sets (PJ-26).
    pub fn material(name: impl Into<String>, rendered: Arc<RenderedMaterial>) -> ScreenImage {
        let preview = rendered.channel(Channel::BaseColor).map(|c| to_tex(c.levels().level(0))).unwrap_or(Tex { w: 1, h: 1, px: vec![[0.5, 0.5, 0.5, 1.0]] });
        ScreenImage { name: name.into(), source: Source::Material(rendered), placement: None, repeat: Repeat::Both, preview, texture: None }
    }

    /// Width / height of one repetition.
    fn aspect(&self) -> f32 {
        self.preview.w as f32 / self.preview.h.max(1) as f32
    }

    /// The placement on a view of `viewport` units, fitting the image to it when it has none yet.
    pub fn placement(&mut self, viewport: [f32; 2]) -> ScreenImageTransform {
        let aspect = self.aspect();
        let t = *self.placement.get_or_insert_with(|| ScreenImageTransform::fitted(aspect, viewport));
        ScreenImageTransform { repeat: self.repeat, ..t }
    }

    /// The placement if the image has been shown.
    pub fn placed(&self) -> Option<ScreenImageTransform> {
        self.placement.map(|t| ScreenImageTransform { repeat: self.repeat, ..t })
    }

    /// Back to the image fitted to the view, unturned (PT-15 reset).
    pub fn reset(&mut self) {
        self.placement = None;
    }

    /// The channels it paints.
    pub fn channels(&self) -> Vec<Channel> {
        match &self.source {
            Source::Picture { .. } => vec![Channel::BaseColor],
            Source::Material(m) => m.channels().collect(),
        }
    }

    /// The stencil factor at `placement` (pictures only: a material is no stencil).
    pub fn stencil(&self, placement: ScreenImageTransform) -> Option<Stencil<'_>> {
        match &self.source {
            Source::Picture { stencil, .. } => Some(Stencil { image: stencil, transform: placement }),
            Source::Material(_) => None,
        }
    }

    /// What a projection stroke through this image paints onto `surface` seen through `view`.
    pub fn projected<'a>(&'a self, surface: &'a SurfaceMap, view: &'a dyn ViewProjector, placement: ScreenImageTransform) -> ProjectedPaint<'a> {
        let p = ProjectedPaint::new(surface, view, placement, Filter::Smooth);
        match &self.source {
            Source::Picture { colour, colour_levels, grey, grey_levels, .. } => {
                p.with_channel(Channel::BaseColor, ImageLevels::new(colour, colour_levels)).with_mask(ImageLevels::new(grey, grey_levels))
            }
            Source::Material(m) => m.channels().fold(p, |p, c| match m.channel(c) {
                Some(t) => p.with_channel(c, t.levels()),
                None => p,
            }),
        }
    }
}

/// The session's stencil and projection image.
#[derive(Default)]
pub struct ScreenImages {
    pub stencil: Option<ScreenImage>,
    pub projection: Option<ScreenImage>,
    /// The stencil is hidden from the view while its key is held (it still masks strokes).
    pub hidden: bool,
    drag: Option<(ImageSlot, GizmoDrag)>,
}

impl ScreenImages {
    pub fn get(&self, slot: ImageSlot) -> Option<&ScreenImage> {
        match slot {
            ImageSlot::Stencil => self.stencil.as_ref(),
            ImageSlot::Projection => self.projection.as_ref(),
        }
    }

    pub fn get_mut(&mut self, slot: ImageSlot) -> Option<&mut ScreenImage> {
        match slot {
            ImageSlot::Stencil => self.stencil.as_mut(),
            ImageSlot::Projection => self.projection.as_mut(),
        }
    }

    /// Puts `image` into `slot` (replacing what was there).
    pub fn set(&mut self, slot: ImageSlot, image: Option<ScreenImage>) {
        self.drag = None;
        match slot {
            ImageSlot::Stencil => self.stencil = image,
            ImageSlot::Projection => self.projection = image,
        }
    }

    /// A gizmo drag on `slot`'s image: starts at `at` when `started`, else continues to `at`. Returns whether the
    /// image moved.
    pub fn drag(&mut self, slot: ImageSlot, action: GizmoAction, at: [f32; 2], started: bool, snap: bool, viewport: [f32; 2]) -> bool {
        let Some(image) = self.get_mut(slot) else { return false };
        let placement = image.placement(viewport);
        let drag = match self.drag {
            Some((s, d)) if s == slot && d.action == action && !started => d,
            _ => {
                let d = GizmoDrag::begin(action, placement, at);
                self.drag = Some((slot, d));
                d
            }
        };
        let moved = drag.update(at, snap);
        let Some(image) = self.get_mut(slot) else { return false };
        image.placement = Some(moved);
        moved != placement
    }

    /// Fits images not placed yet to a view of `viewport` units.
    pub fn place(&mut self, viewport: [f32; 2]) {
        for image in [self.stencil.as_mut(), self.projection.as_mut()].into_iter().flatten() {
            image.placement(viewport);
        }
    }

    /// The stencil's factor for strokes, once it is placed.
    pub fn stencil_factor(&self) -> Option<Stencil<'_>> {
        let image = self.stencil.as_ref()?;
        image.stencil(image.placed()?)
    }

    /// The gizmo drag ended.
    pub fn end_drag(&mut self) {
        self.drag = None;
    }
}

fn luminance(p: [f32; 4]) -> [f32; 4] {
    let l = (0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2]).clamp(0.0, 1.0);
    [l, l, l, p[3]]
}

fn to_tex(t: &dyn Texels) -> Tex {
    let (w, h) = t.size();
    Tex { w: w as usize, h: h as usize, px: (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| t.texel(x, y)).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_is_fitted_once_and_reset_on_request() {
        let tex = Tex { w: 4, h: 2, px: vec![[1.0, 0.0, 0.0, 1.0]; 8] };
        let mut img = ScreenImage::picture("red", tex).unwrap();
        assert!(img.placed().is_none());
        let t = img.placement([400.0, 300.0]);
        assert_eq!(t.size, [400.0, 200.0]);
        let mut images = ScreenImages { stencil: Some(img), ..Default::default() };
        assert!(!images.drag(ImageSlot::Stencil, GizmoAction::Move, [10.0, 10.0], true, false, [400.0, 300.0]));
        assert!(images.drag(ImageSlot::Stencil, GizmoAction::Move, [30.0, 10.0], false, false, [400.0, 300.0]));
        assert_eq!(images.stencil.as_ref().and_then(ScreenImage::placed).map(|t| t.centre), Some([220.0, 150.0]));
        images.end_drag();
        let img = images.stencil.as_mut().unwrap();
        img.reset();
        assert_eq!(img.placement([400.0, 300.0]).centre, [200.0, 150.0]);
        assert_eq!(img.channels(), vec![Channel::BaseColor]);
        let placed = img.placement([400.0, 300.0]);
        assert!(img.stencil(placed).is_some());
        assert!(!images.drag(ImageSlot::Projection, GizmoAction::Move, [0.0; 2], true, false, [400.0, 300.0]), "nothing to drag");
        assert!(ScreenImage::picture("empty", Tex { w: 0, h: 0, px: Vec::new() }).is_err());
    }
}
